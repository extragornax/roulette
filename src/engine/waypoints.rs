use rand::Rng;
use std::f64::consts::PI;

pub fn generate_loop_waypoints(
    start_lat: f64,
    start_lon: f64,
    distance_km: f64,
    direction_deg: f64,
    forced_waypoints: &[(f64, f64)],
) -> Vec<(f64, f64)> {
    let mut rng = rand::rng();
    let n = ((distance_km / 30.0) as usize).clamp(2, 8);
    let radius_km = distance_km / (2.0 * PI);
    let radius_deg = radius_km / 111.0;

    let dir_rad = direction_deg.to_radians();

    let mut wps = Vec::with_capacity(n);
    for i in 0..n {
        let angle = dir_rad + 2.0 * PI * (i as f64) / (n as f64);
        let perturbation: f64 = rng.random_range(0.7..1.3);
        let r = radius_deg * perturbation;

        let lat = start_lat + r * angle.cos();
        let lon = start_lon + r * angle.sin() / start_lat.to_radians().cos();
        wps.push((lat, lon));
    }

    if !forced_waypoints.is_empty() {
        insert_forced_waypoints(&mut wps, forced_waypoints, start_lat, start_lon);
    }

    wps
}

pub fn generate_oneway_waypoints(
    start_lat: f64,
    start_lon: f64,
    distance_km: f64,
    direction_deg: f64,
    forced_waypoints: &[(f64, f64)],
) -> Vec<(f64, f64)> {
    let mut rng = rand::rng();
    let n = ((distance_km / 30.0) as usize).clamp(2, 8);
    let total_deg = (distance_km / 111.0);
    let dir_rad = direction_deg.to_radians();

    let mut wps = Vec::with_capacity(n);
    for i in 1..=n {
        let frac = i as f64 / (n as f64 + 1.0);
        let lateral: f64 = rng.random_range(-0.2..0.2);

        let along = total_deg * frac;
        let lat = start_lat + along * dir_rad.cos() + lateral * total_deg * dir_rad.sin();
        let lon = start_lon
            + (along * dir_rad.sin() + lateral * total_deg * dir_rad.cos())
                / start_lat.to_radians().cos();
        wps.push((lat, lon));
    }

    if !forced_waypoints.is_empty() {
        insert_forced_waypoints(&mut wps, forced_waypoints, start_lat, start_lon);
    }

    wps
}

pub fn rotate_waypoints(wps: &[(f64, f64)], center: (f64, f64), angle_deg: f64) -> Vec<(f64, f64)> {
    let a = angle_deg.to_radians();
    wps.iter()
        .map(|(lat, lon)| {
            let dlat = lat - center.0;
            let dlon = lon - center.1;
            let new_dlat = dlat * a.cos() - dlon * a.sin();
            let new_dlon = dlat * a.sin() + dlon * a.cos();
            (center.0 + new_dlat, center.1 + new_dlon)
        })
        .collect()
}

pub fn scale_waypoints(wps: &[(f64, f64)], center: (f64, f64), factor: f64) -> Vec<(f64, f64)> {
    wps.iter()
        .map(|(lat, lon)| {
            let dlat = lat - center.0;
            let dlon = lon - center.1;
            (center.0 + dlat * factor, center.1 + dlon * factor)
        })
        .collect()
}

fn insert_forced_waypoints(
    wps: &mut Vec<(f64, f64)>,
    forced: &[(f64, f64)],
    _start_lat: f64,
    _start_lon: f64,
) {
    for &fw in forced {
        let best_idx = wps
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (a.0 - fw.0).powi(2) + (a.1 - fw.1).powi(2);
                let db = (b.0 - fw.0).powi(2) + (b.1 - fw.1).powi(2);
                da.partial_cmp(&db).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        wps.insert(best_idx + 1, fw);
    }
}
