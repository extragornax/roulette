use anyhow::Result;
use gpx::Waypoint;

use crate::engine::{
    avoid,
    brouter::{BRouterClient, Profile},
    gpx_util::{self, RouteStats},
    waypoints,
};

pub const TOLERANCE_PCT: f64 = 10.0;
pub const MAX_ITER: usize = 3;
pub const OVERLAP_THRESHOLD_PCT: f64 = 30.0;

pub struct GenerateInput {
    pub start: (f64, f64),
    pub distance_km: f64,
    pub dplus_max: Option<f64>,
    pub profile: Profile,
    pub is_loop: bool,
    pub forced: Vec<(f64, f64)>,
    pub avoid_points: Vec<(f64, f64)>,
    pub seed_direction: Option<f64>,
}

pub struct GenerateOutput {
    pub gpx: String,
    pub stats: RouteStats,
    pub waypoints: Vec<(f64, f64)>,
    pub warnings: Vec<String>,
}

pub async fn generate(client: &BRouterClient, input: &GenerateInput) -> Result<GenerateOutput> {
    let mut direction = input
        .seed_direction
        .unwrap_or_else(|| rand::random::<f64>() * 360.0);
    let mut scale = 1.0_f64;

    let mut best: Option<(f64, Vec<(f64, f64)>, Vec<Vec<Waypoint>>, RouteStats)> = None;
    let mut last_err: Option<anyhow::Error> = None;

    for iter in 0..MAX_ITER {
        let base_wps = build_waypoints(input, direction);
        let wps = waypoints::scale_waypoints(&base_wps, input.start, scale);

        let route_points = assemble_route_points(input, &wps);
        let segments = match client.route_through(&route_points, input.profile).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(iter, "brouter call failed: {e:#}");
                last_err = Some(e);
                continue;
            }
        };

        let flat: Vec<Waypoint> = segments.iter().flat_map(|s| s.iter().cloned()).collect();
        if flat.is_empty() {
            continue;
        }
        let stats = gpx_util::compute_stats(&flat, input.start);

        let dist_ratio = stats.distance_km / input.distance_km;
        let in_tolerance =
            (1.0 - TOLERANCE_PCT / 100.0..=1.0 + TOLERANCE_PCT / 100.0).contains(&dist_ratio);
        let dplus_ok = input.dplus_max.map(|m| stats.dplus_m <= m).unwrap_or(true);
        let overlap = avoid::overlap_pct(&flat, &input.avoid_points);
        let overlap_ok = overlap <= OVERLAP_THRESHOLD_PCT;

        let candidate_score = score(&stats, input, overlap);
        let candidate = (candidate_score, wps.clone(), segments.clone(), stats.clone());
        if best.as_ref().map_or(true, |b| candidate.0 < b.0) {
            best = Some(candidate);
        }

        if in_tolerance && dplus_ok && overlap_ok {
            break;
        }

        if !in_tolerance && dist_ratio > 0.0 {
            scale *= 1.0 / dist_ratio;
        }
        if !overlap_ok {
            direction = (direction + 90.0) % 360.0;
        }
        if !dplus_ok {
            scale *= 0.9;
        }

        tracing::debug!(
            iter,
            dist = stats.distance_km,
            dplus = stats.dplus_m,
            overlap,
            "constraint miss"
        );
    }

    let (_, final_wps, segments, stats) = best
        .ok_or_else(|| last_err.unwrap_or_else(|| anyhow::anyhow!("no route generated")))?;

    let mut warnings = Vec::new();
    let dist_ratio = stats.distance_km / input.distance_km;
    if !(1.0 - TOLERANCE_PCT / 100.0..=1.0 + TOLERANCE_PCT / 100.0).contains(&dist_ratio) {
        warnings.push(format!(
            "distance {:.1}km hors tolérance (cible {:.0}km ±{:.0}%)",
            stats.distance_km, input.distance_km, TOLERANCE_PCT
        ));
    }
    if let Some(max) = input.dplus_max {
        if stats.dplus_m > max {
            warnings.push(format!(
                "D+ {:.0}m au-dessus du max demandé ({:.0}m)",
                stats.dplus_m, max
            ));
        }
    }
    let overlap = avoid::overlap_pct(
        &segments.iter().flat_map(|s| s.iter().cloned()).collect::<Vec<_>>(),
        &input.avoid_points,
    );
    if overlap > OVERLAP_THRESHOLD_PCT {
        warnings.push(format!(
            "{:.0}% du parcours recouvre des routes déjà faites",
            overlap
        ));
    }

    let gpx = gpx_util::assemble_gpx(&segments);

    Ok(GenerateOutput {
        gpx,
        stats,
        waypoints: final_wps,
        warnings,
    })
}

fn build_waypoints(input: &GenerateInput, direction_deg: f64) -> Vec<(f64, f64)> {
    if input.is_loop {
        waypoints::generate_loop_waypoints(
            input.start.0,
            input.start.1,
            input.distance_km,
            direction_deg,
            &input.forced,
        )
    } else {
        waypoints::generate_oneway_waypoints(
            input.start.0,
            input.start.1,
            input.distance_km,
            direction_deg,
            &input.forced,
        )
    }
}

fn assemble_route_points(input: &GenerateInput, wps: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut pts = Vec::with_capacity(wps.len() + 2);
    pts.push(input.start);
    pts.extend_from_slice(wps);
    if input.is_loop {
        pts.push(input.start);
    }
    pts
}

fn score(stats: &RouteStats, input: &GenerateInput, overlap: f64) -> f64 {
    let dist_err = ((stats.distance_km - input.distance_km).abs() / input.distance_km).max(0.0);
    let dplus_err = match input.dplus_max {
        Some(max) if stats.dplus_m > max => (stats.dplus_m - max) / max,
        _ => 0.0,
    };
    let overlap_err = (overlap / 100.0).max(0.0);
    dist_err + dplus_err + overlap_err
}
