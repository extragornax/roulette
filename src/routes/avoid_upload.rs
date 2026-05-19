use std::sync::Arc;

use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use uuid::Uuid;

use crate::engine::avoid;
use crate::state::AppState;

#[derive(Serialize)]
pub struct UploadResponse {
    pub session_id: String,
    pub points: usize,
}

pub async fn upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut all_points: Vec<(f64, f64)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart: {e}")))?
    {
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("read field: {e}")))?;
        let pts = avoid::extract_points_from_gpx_bytes(&data)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("gpx parse: {e:#}")))?;
        all_points.extend(pts);
    }

    if all_points.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "no points in uploaded GPX".into()));
    }

    let session_id = Uuid::new_v4().to_string();
    let count = all_points.len();
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db lock: {e}")))?;
        avoid::store_session(&conn, &session_id, &all_points)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("store: {e:#}")))?;
    }

    Ok(Json(UploadResponse {
        session_id,
        points: count,
    }))
}
