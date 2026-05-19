use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct GeocodeQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct GeocodeResponse {
    pub lat: f64,
    pub lon: f64,
}

static HTTP: OnceCell<reqwest::Client> = OnceCell::const_new();

async fn http() -> &'static reqwest::Client {
    HTTP.get_or_init(|| async {
        reqwest::Client::builder()
            .user_agent("roulette-velo/1.0 (extragornax.fr)")
            .timeout(Duration::from_secs(10))
            .build()
            .expect("building reqwest client")
    })
    .await
}

pub async fn geocode(
    State(state): State<Arc<AppState>>,
    Query(q): Query<GeocodeQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let query = q.q.trim().to_string();
    if query.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty query".into()));
    }

    {
        let cache = state.geocode_cache.lock().map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("cache lock: {e}"))
        })?;
        if let Some(hit) = cache.get(&query).copied() {
            return Ok(Json(GeocodeResponse {
                lat: hit.0,
                lon: hit.1,
            }));
        }
    }

    let url = format!(
        "https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1",
        urlencode(&query)
    );
    let resp = http()
        .await
        .get(&url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("nominatim: {e}")))?;

    if !resp.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("nominatim status {}", resp.status()),
        ));
    }

    #[derive(Deserialize)]
    struct Hit {
        lat: String,
        lon: String,
    }

    let hits: Vec<Hit> = resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("nominatim json: {e}")))?;
    let first = hits
        .first()
        .ok_or((StatusCode::NOT_FOUND, "no result".into()))?;
    let lat: f64 = first
        .lat
        .parse()
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("parse lat: {e}")))?;
    let lon: f64 = first
        .lon
        .parse()
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("parse lon: {e}")))?;

    if let Ok(mut cache) = state.geocode_cache.lock() {
        cache.insert(query, (lat, lon));
    }

    Ok(Json(GeocodeResponse { lat, lon }))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
