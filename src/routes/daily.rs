use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::engine::{
    brouter::{BRouterClient, Profile},
    constraints::{self, GenerateInput},
};
use crate::state::AppState;

pub const CITIES: &[(&str, f64, f64)] = &[
    ("paris", 48.8566, 2.3522),
    ("lyon", 45.7640, 4.8357),
    ("marseille", 43.2965, 5.3698),
    ("bordeaux", 44.8378, -0.5792),
    ("toulouse", 43.6047, 1.4442),
    ("nantes", 47.2184, -1.5536),
    ("strasbourg", 48.5734, 7.7521),
    ("lille", 50.6292, 3.0573),
    ("montpellier", 43.6108, 3.8767),
    ("rennes", 48.1173, -1.6778),
];

#[derive(Deserialize)]
pub struct DailyQuery {
    pub city: String,
}

#[derive(Serialize)]
pub struct DailyResponse {
    pub city: String,
    pub date: String,
    pub gpx: String,
    pub distance_km: f64,
    pub dplus_m: f64,
    pub waypoints: Vec<[f64; 2]>,
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DailyQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let city = q.city.to_lowercase();
    let date = Utc::now().format("%Y-%m-%d").to_string();

    if let Some(row) = read_cached(&state, &city, &date)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?
    {
        return Ok(Json(row));
    }

    let (_, lat, lon) = CITIES
        .iter()
        .find(|(name, _, _)| *name == city.as_str())
        .ok_or((StatusCode::NOT_FOUND, "unknown city".into()))?;

    let direction = seeded_direction(&city, &date);
    let input = GenerateInput {
        start: (*lat, *lon),
        distance_km: 80.0,
        dplus_max: None,
        profile: Profile::Trekking,
        is_loop: true,
        forced: Vec::new(),
        avoid_points: Vec::new(),
        seed_direction: Some(direction),
    };

    let client = BRouterClient::new(state.brouter_url.clone(), state.needs_rate_limit);
    let out = constraints::generate(&client, &input)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;

    let wp_json = serde_json::to_string(&out.waypoints).unwrap_or_else(|_| "[]".into());
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db lock: {e}")))?;
        conn.execute(
            "INSERT OR REPLACE INTO daily_routes
               (city, date, gpx, distance_km, dplus_m, waypoints)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                city,
                date,
                out.gpx.as_bytes(),
                out.stats.distance_km,
                out.stats.dplus_m,
                wp_json
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("insert: {e}")))?;
    }

    Ok(Json(DailyResponse {
        city,
        date,
        gpx: out.gpx,
        distance_km: out.stats.distance_km,
        dplus_m: out.stats.dplus_m,
        waypoints: out.waypoints.into_iter().map(|p| [p.0, p.1]).collect(),
    }))
}

fn read_cached(
    state: &Arc<AppState>,
    city: &str,
    date: &str,
) -> anyhow::Result<Option<DailyResponse>> {
    let conn = state.db.lock().map_err(|e| anyhow::anyhow!("db lock: {e}"))?;
    let mut stmt = conn.prepare(
        "SELECT gpx, distance_km, dplus_m, waypoints FROM daily_routes
         WHERE city = ?1 AND date = ?2",
    )?;
    let mut rows = stmt.query(rusqlite::params![city, date])?;
    if let Some(row) = rows.next()? {
        let gpx_blob: Vec<u8> = row.get(0)?;
        let distance_km: f64 = row.get(1)?;
        let dplus_m: f64 = row.get(2)?;
        let wp_json: String = row.get(3)?;
        let wps: Vec<[f64; 2]> = serde_json::from_str(&wp_json).unwrap_or_default();
        return Ok(Some(DailyResponse {
            city: city.into(),
            date: date.into(),
            gpx: String::from_utf8_lossy(&gpx_blob).to_string(),
            distance_km,
            dplus_m,
            waypoints: wps,
        }));
    }
    Ok(None)
}

fn seeded_direction(city: &str, date: &str) -> f64 {
    let mut h: u64 = 1469598103934665603;
    for b in city.bytes().chain(date.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    ((h % 3600) as f64) / 10.0
}
