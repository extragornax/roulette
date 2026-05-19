use anyhow::{Context, Result};
use rusqlite::Connection;

pub fn init(path: &str) -> Result<Connection> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).context("creating DB parent directory")?;
    }

    let conn = Connection::open(path).context("opening SQLite database")?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA busy_timeout=5000;
         PRAGMA foreign_keys=ON;",
    )
    .context("setting SQLite pragmas")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily_routes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            city        TEXT    NOT NULL,
            date        TEXT    NOT NULL,
            gpx         BLOB   NOT NULL,
            distance_km REAL   NOT NULL,
            dplus_m     REAL   NOT NULL,
            waypoints   TEXT   NOT NULL,
            created_at  TEXT   NOT NULL DEFAULT (datetime('now')),
            UNIQUE(city, date)
        );

        CREATE TABLE IF NOT EXISTS avoid_sessions (
            id         TEXT PRIMARY KEY,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS avoid_segments (
            session_id TEXT    NOT NULL REFERENCES avoid_sessions(id),
            lat        REAL    NOT NULL,
            lon        REAL    NOT NULL,
            seq        INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_avoid_segments_session ON avoid_segments(session_id);
        CREATE INDEX IF NOT EXISTS idx_daily_routes_lookup ON daily_routes(city, date);",
    )
    .context("creating database schema")?;

    Ok(conn)
}

pub fn cleanup_old_sessions(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM avoid_segments WHERE session_id IN (
            SELECT id FROM avoid_sessions WHERE created_at < datetime('now', '-24 hours')
        )",
        [],
    )?;
    conn.execute(
        "DELETE FROM avoid_sessions WHERE created_at < datetime('now', '-24 hours')",
        [],
    )?;
    Ok(())
}
