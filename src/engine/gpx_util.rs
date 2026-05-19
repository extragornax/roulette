use anyhow::{Context, Result};
use geo::HaversineDistance;
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};

#[derive(Debug, Clone, serde::Serialize)]
pub struct RouteStats {
    pub distance_km: f64,
    pub dplus_m: f64,
    pub dminus_m: f64,
    pub estimated_duration_h: f64,
    pub dominant_direction: String,
    pub greenway_pct: f64,
    pub elevations: Vec<(f64, f64)>,
}

pub fn parse_gpx_bytes(data: &[u8]) -> Result<Gpx> {
    gpx::read(std::io::BufReader::new(data)).context("parsing GPX")
}

pub fn extract_points(gpx: &Gpx) -> Vec<Waypoint> {
    gpx.tracks
        .iter()
        .flat_map(|t| &t.segments)
        .flat_map(|s| &s.points)
        .cloned()
        .collect()
}

pub fn compute_stats(points: &[Waypoint], start: (f64, f64)) -> RouteStats {
    let mut distance = 0.0;
    let mut dplus = 0.0;
    let mut dminus = 0.0;
    let mut elevations = Vec::new();
    let mut cum_dist = 0.0;

    for i in 0..points.len() {
        let ele = points[i].elevation.unwrap_or(0.0);

        if i > 0 {
            let p1 = geo::Point::new(points[i - 1].point().x(), points[i - 1].point().y());
            let p2 = geo::Point::new(points[i].point().x(), points[i].point().y());
            let seg_dist = p1.haversine_distance(&p2);
            distance += seg_dist;
            cum_dist += seg_dist;

            let prev_ele = points[i - 1].elevation.unwrap_or(0.0);
            let diff = ele - prev_ele;
            if diff > 0.0 {
                dplus += diff;
            } else {
                dminus += diff.abs();
            }
        }

        if i % 5 == 0 || i == points.len() - 1 {
            elevations.push((cum_dist / 1000.0, ele));
        }
    }

    let distance_km = distance / 1000.0;

    let avg_grade = if distance_km > 0.0 {
        dplus / (distance_km * 1000.0) * 100.0
    } else {
        0.0
    };
    let speed = (20.0 - 2.0 * avg_grade).max(8.0);
    let estimated_duration_h = distance_km / speed;

    let last = points.last().map(|p| (p.point().y(), p.point().x()));
    let dominant_direction = match last {
        Some((lat, lon)) => bearing_to_cardinal(bearing(start.0, start.1, lat, lon)),
        None => "N".to_string(),
    };

    RouteStats {
        distance_km,
        dplus_m: dplus,
        dminus_m: dminus,
        estimated_duration_h,
        dominant_direction,
        greenway_pct: 0.0,
        elevations,
    }
}

pub fn assemble_gpx(segments: &[Vec<Waypoint>]) -> String {
    let mut gpx = Gpx {
        version: GpxVersion::Gpx11,
        ..Default::default()
    };

    let mut all_points = Vec::new();
    for seg in segments {
        all_points.extend(seg.iter().cloned());
    }

    let segment = TrackSegment { points: all_points };
    let track = Track {
        segments: vec![segment],
        ..Default::default()
    };
    gpx.tracks.push(track);

    let mut buf = Vec::new();
    gpx::write(&gpx, &mut buf).unwrap_or_default();
    String::from_utf8_lossy(&buf).to_string()
}

fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (lat1, lon1, lat2, lon2) = (
        lat1.to_radians(),
        lon1.to_radians(),
        lat2.to_radians(),
        lon2.to_radians(),
    );
    let dlon = lon2 - lon1;
    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

fn bearing_to_cardinal(deg: f64) -> String {
    let dirs = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let idx = ((deg + 22.5) / 45.0) as usize % 8;
    dirs[idx].to_string()
}
