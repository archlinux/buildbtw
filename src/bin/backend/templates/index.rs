use color_eyre::Result;

use crate::{entities, templates};

pub fn render_index_page(user: &Option<entities::users::Model>) -> Result<String> {
    let mut ctx = tera::Context::default();
    ctx.insert("user", &user);
    templates::render("routes/index.html", ctx)
}
