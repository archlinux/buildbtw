use color_eyre::Result;

use crate::{csp, entities, templates};

pub fn render_index_page(
    user: Option<&entities::users::Model>,
    nonce: &csp::Nonce,
) -> Result<String> {
    let mut ctx = tera::Context::default();
    ctx.insert("user", &user);
    templates::render("routes/index.html", ctx, nonce)
}
