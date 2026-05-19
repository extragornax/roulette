use anyhow::{Context, Result};
use geo::HaversineDistance;
use gpx::Waypoint;
use rusqlite::Connection;

use crate::engine::gpx_util;

const BUFFER_M: f64 = 200.0;

pub fn store_session(conn: &Connection, session_id: &str, points: &[(f64, f64)]) -> Result<()> {
    conn.execute(
        "INSERT INTO avoid_sessions (id) VALUES (?1)",
        [session_id],
    )?;
    let tx_sql = "INSERT INTO avoid_segments (session_id, lat, lon, seq) VALUES (?1, ?2, ?3, ?4)";
    let mut stmt = conn.prepare(tx_sql)?;
    for (i, (lat, lon)) in points.iter().enumerate() {
        stmt.execute(rusqlite::params![session_id, lat, lon, i as i64])?;
    }
    Ok(())
}

pub fn load_session_points(conn: &Connection, session_id: &str) -> Result<Vec<(f64, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT lat, lon FROM avoid_segments WHERE session_id = ?1 ORDER BY seq ASC",
    )?;
    let rows = stmt
        .query_map([session_id], |row| {
            Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn extract_points_from_gpx_bytes(data: &[u8]) -> Result<Vec<(f64, f64)>> {
    let gpx = gpx_util::parse_gpx_bytes(data).context("parsing avoid GPX")?;
    let pts = gpx_util::extract_points(&gpx);
    Ok(pts
        .iter()
        .map(|p| (p.point().y(), p.point().x()))
        .collect())
}

pub fn overlap_pct(route: &[Waypoint], avoid_points: &[(f64, f64)]) -> f64 {
    if route.is_empty() || avoid_points.is_empty() {
        return 0.0;
    }

    let mut overlapping = 0usize;
    for wp in route {
        let p = geo::Point::new(wp.point().x(), wp.point().y());
        let near = avoid_points.iter().any(|(lat, lon)| {
            let q = geo::Point::new(*lon, *lat);
            p.haversine_distance(&q) <= BUFFER_M
        });
        if near {
            overlapping += 1;
        }
    }

    (overlapping as f64 / route.len() as f64) * 100.0
}
