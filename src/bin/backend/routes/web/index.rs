use axum::response::{Html, IntoResponse};
use buildbtw::web;

use crate::from_request;

pub async fn index(
    _: web::builds::Index,
    session: Option<from_request::AuthUser>,
) -> impl IntoResponse {
    let text = match session {
        Some(s) => format!("Logged in as {}", s.user.username),
        None => {
            let login_url = &web::oidc::StartLogin {}.to_string();
            format!("Bonjour! Feel free to <a href=\"{login_url}\">login</a>")
        }
    };

    Html(format!("<div id=\"content\">{text}</div>"))
}
