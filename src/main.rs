mod db;
mod engine;
mod routes;
mod state;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use tokio::signal;
use tower_http::trace::TraceLayer;

use crate::engine::brouter;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,roulette=info".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let db_path =
        std::env::var("DB_PATH").unwrap_or_else(|_| "data/roulette.db".into());
    let brouter_url =
        std::env::var("BROUTER_URL").unwrap_or_else(|_| "https://brouter.de/brouter".into());

    let conn = db::init(&db_path).context("init DB")?;
    db::cleanup_old_sessions(&conn).ok();

    let state = Arc::new(AppState {
        db: Mutex::new(conn),
        needs_rate_limit: brouter::detect_rate_limit(&brouter_url),
        brouter_url,
        geocode_cache: Mutex::new(HashMap::new()),
    });

    spawn_cleanup_task(state.clone());
    spawn_daily_task(state.clone());

    let app = Router::new()
        .route("/", get(routes::pages::index))
        .route("/daily", get(routes::pages::daily_page))
        .route("/static/app.css", get(routes::pages::app_css))
        .route("/api/generate", post(routes::generate::generate))
        .route(
            "/api/avoid/upload",
            post(routes::avoid_upload::upload)
                .layer(DefaultBodyLimit::max(20 * 1024 * 1024)),
        )
        .route("/api/daily", get(routes::daily::get))
        .route("/api/geocode", get(routes::geocode::geocode))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!("listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum serve")?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("shutdown signal received");
}

fn spawn_cleanup_task(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Ok(conn) = state.db.lock() {
                if let Err(e) = db::cleanup_old_sessions(&conn) {
                    tracing::warn!("cleanup failed: {e:#}");
                }
            }
        }
    });
}

fn spawn_daily_task(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            let sleep = seconds_until_next_utc_midnight();
            tokio::time::sleep(Duration::from_secs(sleep)).await;
            tracing::info!("regenerating daily routes");
            for (city, lat, lon) in routes::daily::CITIES {
                let input = crate::engine::constraints::GenerateInput {
                    start: (*lat, *lon),
                    distance_km: 80.0,
                    dplus_max: None,
                    profile: brouter::Profile::Trekking,
                    is_loop: true,
                    forced: Vec::new(),
                    avoid_points: Vec::new(),
                    seed_direction: None,
                };
                let client = brouter::BRouterClient::new(
                    state.brouter_url.clone(),
                    state.needs_rate_limit,
                );
                let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
                match crate::engine::constraints::generate(&client, &input).await {
                    Ok(out) => {
                        let wp_json =
                            serde_json::to_string(&out.waypoints).unwrap_or_else(|_| "[]".into());
                        if let Ok(conn) = state.db.lock() {
                            let _ = conn.execute(
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
                            );
                        }
                    }
                    Err(e) => tracing::warn!("daily {city} failed: {e:#}"),
                }
            }
        }
    });
}

fn seconds_until_next_utc_midnight() -> u64 {
    let now = chrono::Utc::now();
    let tomorrow = (now + chrono::Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    (tomorrow - now).num_seconds().max(60) as u64
}
