use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::Connection;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub brouter_url: String,
    pub needs_rate_limit: bool,
    pub geocode_cache: Mutex<HashMap<String, (f64, f64)>>,
}
