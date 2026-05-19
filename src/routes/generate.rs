use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::engine::{
    avoid,
    brouter::{BRouterClient, Profile},
    constraints::{self, GenerateInput},
    gpx_util::RouteStats,
};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub start: [f64; 2],
    pub distance_km: f64,
    #[serde(default)]
    pub dplus_max: Option<f64>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default = "default_loop", rename = "loop")]
    pub is_loop: bool,
    #[serde(default)]
    pub waypoints: Vec<[f64; 2]>,
    #[serde(default)]
    pub avoid_session: Option<String>,
}

fn default_loop() -> bool {
    true
}

#[derive(Serialize)]
pub struct GenerateResponse {
    pub gpx: String,
    pub stats: RouteStats,
    pub waypoints: Vec<[f64; 2]>,
    pub warnings: Vec<String>,
}

pub async fn generate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GenerateRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let avoid_points = match req.avoid_session.as_ref() {
        Some(sid) => {
            let conn = state
                .db
                .lock()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db lock: {e}")))?;
            avoid::load_session_points(&conn, sid)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("avoid session: {e:#}")))?
        }
        None => Vec::new(),
    };

    let input = GenerateInput {
        start: (req.start[0], req.start[1]),
        distance_km: req.distance_km,
        dplus_max: req.dplus_max,
        profile: req
            .profile
            .as_deref()
            .map(Profile::parse)
            .unwrap_or(Profile::Trekking),
        is_loop: req.is_loop,
        forced: req.waypoints.iter().map(|p| (p[0], p[1])).collect(),
        avoid_points,
        seed_direction: None,
    };

    let client = BRouterClient::new(state.brouter_url.clone(), state.needs_rate_limit);

    let out = constraints::generate(&client, &input)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;

    Ok(Json(GenerateResponse {
        gpx: out.gpx,
        stats: out.stats,
        waypoints: out.waypoints.into_iter().map(|p| [p.0, p.1]).collect(),
        warnings: out.warnings,
    }))
}
