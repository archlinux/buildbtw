use axum::response::IntoResponse;
use buildbtw::web;

pub async fn index(_: web::builds::Index) -> impl IntoResponse {
    "Bonjour!"
}
