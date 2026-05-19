# roulette

**Roulette Vélo** — randomized cycling route generator under constraints. Pick a start, distance, D+ ceiling; the engine builds a surprise loop (or one-way) routed through [BRouter](https://brouter.de/) and serves the GPX.

Part of the [extragornax.fr](https://extragornax.fr) self-hosted cycling toolkit. Target domain: `roulette.extragornax.fr`.

## Features

- Loop or one-way generation with N intermediate waypoints (`distance / 30`, clamped 2–8)
- Distance tolerance ±10%, D+ cap, profile (`trekking` / `fastbike` / `safety`)
- Up to 3 forced waypoints
- "Avoid my routes" — upload GPX, 200m buffer, retry on >30% overlap
- Daily challenge: same seeded route per French city, regenerated at UTC midnight
- Permalink share via URL hash
- Local history (20 last) in `localStorage`
- Geocoding proxy (Nominatim, cached)
- Riso/editorial UI (Bricolage / Fraunces / Space Mono, paper/ink/vermilion palette)

## Stack

- **Rust 2024**, **Axum 0.7**, **Tokio**
- **rusqlite** (bundled) — daily cache + avoid sessions
- **gpx 0.10** + **geo 0.29** — parsing, haversine
- **reqwest** (rustls) — BRouter, Nominatim
- **Leaflet** + vanilla JS frontend

## Layout

```
src/
  main.rs            Axum setup, graceful shutdown, cleanup + daily background tasks
  state.rs           AppState (DB conn, BRouter URL, geocode cache)
  db.rs              SQLite init, schema, 24h avoid cleanup
  engine/
    waypoints.rs     loop/oneway generation, rotate/scale
    brouter.rs       BRouter HTTP client + profile enum + rate limit
    gpx_util.rs      parse, assemble, stats (distance, D+/D-, bearing)
    avoid.rs         session store, 200m buffer overlap %
    constraints.rs   retry loop (≤3 iter), distance/D+/overlap
  routes/
    pages.rs         GET /, /daily, /static/app.css
    generate.rs      POST /api/generate
    avoid_upload.rs  POST /api/avoid/upload (multipart, 20 MB max)
    daily.rs         GET /api/daily?city=
    geocode.rs       GET /api/geocode?q=
templates/index.html templates/daily.html
static/app.css
Dockerfile docker-compose.yml
```

## Run

### Local

```bash
cargo run --release
```

Then open `http://localhost:3000`.

### Docker Compose

```bash
docker compose up --build
```

Persistent SQLite lives in the `roulette_data` named volume (`/data` inside the container).

## Configuration

| Env var       | Default                          | Notes |
|---------------|----------------------------------|-------|
| `PORT`        | `3000`                           | HTTP port |
| `DB_PATH`     | `data/roulette.db`               | SQLite path (parent auto-created) |
| `BROUTER_URL` | `https://brouter.de/brouter`     | Self-host friendly; rate limit auto-detected on `brouter.de` |
| `RUST_LOG`    | `info,roulette=info`             | `tracing-subscriber` env filter |

## API

| Method | Path                   | Body / Query | Returns |
|--------|------------------------|--------------|---------|
| GET    | `/`                    |              | HTML form + map |
| GET    | `/daily`               |              | HTML grid of city routes |
| POST   | `/api/generate`        | JSON `{start, distance_km, dplus_max?, profile?, loop?, waypoints?, avoid_session?}` | `{gpx, stats, waypoints, warnings}` |
| POST   | `/api/avoid/upload`    | multipart GPX | `{session_id, points}` |
| GET    | `/api/daily?city=`     | `paris|lyon|...|rennes` | `{city, date, gpx, distance_km, dplus_m, waypoints}` |
| GET    | `/api/geocode?q=`      | text         | `{lat, lon}` |

## Database

`daily_routes(city, date YYYY-MM-DD, gpx BLOB, distance_km, dplus_m, waypoints JSON)` — unique on `(city, date)`.

`avoid_sessions(id UUID, created_at)` + `avoid_segments(session_id, lat, lon, seq)` — TTL 24 h, cleaned hourly.

WAL mode, `busy_timeout=5000`, `foreign_keys=ON`.

## Constraints

- BRouter requests serialized; 200 ms gap if hitting `brouter.de`
- Nominatim User-Agent `roulette-velo/1.0 (extragornax.fr)`
- Upload limit 20 MB
- No auth, no accounts — stateless except daily cache + temp avoid sessions

## License

TBD.
