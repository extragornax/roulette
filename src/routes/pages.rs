use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};

const INDEX: &str = include_str!("../../templates/index.html");
const DAILY: &str = include_str!("../../templates/daily.html");
const APP_CSS: &str = include_str!("../../static/app.css");

pub async fn index() -> Html<&'static str> {
    Html(INDEX)
}

pub async fn daily_page() -> Html<&'static str> {
    Html(DAILY)
}

pub async fn app_css() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        APP_CSS,
    )
        .into_response()
}
