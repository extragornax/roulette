use anyhow::{Context, Result, anyhow};
use gpx::{Gpx, Waypoint};
use std::time::Duration;

use crate::engine::gpx_util;

const RATE_LIMIT_MS: u64 = 200;

#[derive(Clone, Copy, Debug)]
pub enum Profile {
    Trekking,
    Fastbike,
    Safety,
}

impl Profile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::Trekking => "trekking",
            Profile::Fastbike => "fastbike",
            Profile::Safety => "safety",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "fastbike" => Profile::Fastbike,
            "safety" => Profile::Safety,
            _ => Profile::Trekking,
        }
    }
}

pub struct BRouterClient {
    base_url: String,
    rate_limit: bool,
    http: reqwest::Client,
}

impl BRouterClient {
    pub fn new(base_url: String, rate_limit: bool) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("roulette-velo/1.0 (extragornax.fr)")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("building reqwest client");
        Self {
            base_url,
            rate_limit,
            http,
        }
    }

    pub async fn route_segment(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        profile: Profile,
    ) -> Result<Vec<Waypoint>> {
        if self.rate_limit {
            tokio::time::sleep(Duration::from_millis(RATE_LIMIT_MS)).await;
        }

        let lonlats = format!("{},{}|{},{}", from.1, from.0, to.1, to.0);
        let url = format!(
            "{}/brouter?lonlats={}&profile={}&alternativeidx=0&format=gpx",
            self.base_url,
            lonlats,
            profile.as_str()
        );

        let resp = self.http.get(&url).send().await.context("calling BRouter")?;
        let status = resp.status();
        let body = resp.bytes().await.context("reading BRouter body")?;

        if !status.is_success() {
            return Err(anyhow!(
                "BRouter {} returned {}: {}",
                url,
                status,
                String::from_utf8_lossy(&body).chars().take(200).collect::<String>()
            ));
        }

        let gpx: Gpx = gpx_util::parse_gpx_bytes(&body).context("parsing BRouter GPX")?;
        Ok(gpx_util::extract_points(&gpx))
    }

    pub async fn route_through(
        &self,
        points: &[(f64, f64)],
        profile: Profile,
    ) -> Result<Vec<Vec<Waypoint>>> {
        if points.len() < 2 {
            return Err(anyhow!("need at least 2 points to route"));
        }

        let mut segments = Vec::with_capacity(points.len() - 1);
        for pair in points.windows(2) {
            let seg = self.route_segment(pair[0], pair[1], profile).await?;
            segments.push(seg);
        }
        Ok(segments)
    }
}

pub fn detect_rate_limit(url: &str) -> bool {
    url.contains("brouter.de")
}
