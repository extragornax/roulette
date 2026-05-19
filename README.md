# roulette

Cycling route generator. Spins randomized loops or one-way rides from a start point, routes them through [BRouter](https://brouter.de/), and serves the GPX over HTTP.

## Status

Early scaffolding. Working: GPX parsing/stats, waypoint generation, SQLite schema. Missing: HTTP handlers, BRouter client, avoid-segment logic, constraints engine, frontend.

## Stack

- **axum** — HTTP server
- **rusqlite** (bundled) — daily route cache, avoid-segment sessions
- **gpx + geo** — track parsing, haversine distance, elevation stats
- **reqwest** (rustls) — BRouter calls
- **rand** — waypoint perturbation

## Layout

```
src/
  db.rs              SQLite init + schema (daily_routes, avoid_*)
  state.rs           AppState (DB conn, BRouter URL, geocode cache)
  engine/
    mod.rs           module wiring
    waypoints.rs     loop/oneway waypoint generation, rotate/scale
    gpx_util.rs      GPX parse, stats (distance, D+/D-, bearing)
    avoid.rs         (planned) avoided-segment handling
    brouter.rs       (planned) BRouter HTTP client
    constraints.rs   (planned) forced-waypoint insertion, filters
static/              frontend assets
templates/           HTML templates
```

## Build

```bash
cargo build --release
```

Requires Rust 2024 edition (1.85+).

## Run

Not wired yet (no `main.rs`). Once present:

```bash
cargo run --release
```

## Database

SQLite file created on first init via `db::init(path)`. Schema:

- `daily_routes` — one cached route per `(city, date)` with GPX blob + stats
- `avoid_sessions` / `avoid_segments` — temp user-drawn avoid polylines (24h TTL via `cleanup_old_sessions`)

WAL mode, `busy_timeout=5000`, foreign keys on.

## License

TBD.
