use color_eyre::Result;

use crate::{entities, entities::sessions, templates};
use buildbtw::web;

pub fn render_account_overview(user: &entities::users::Model) -> Result<String> {
    let mut ctx = tera::Context::default();
    ctx.insert("user", &user);
    templates::render("routes/account/overview.html", ctx)
}

pub fn render_session_list_page(
    user: &entities::users::Model,
    sessions: Vec<sessions::Model>,
) -> Result<String> {
    let mut ctx = tera::Context::default();
    ctx.insert("user", &user);

    let sessions = sessions
        .iter()
        .map(|session| {
            let url = web::account::SessionRevoke {
                session_id: session.id.0.to_string(),
            }
            .to_string();
            (session, url)
        })
        .collect::<Vec<_>>();
    ctx.insert("sessions", &sessions);

    templates::render("routes/account/session.html", ctx)
}
