use color_eyre::Result;

use crate::{csp, entities::users, templates, web};

pub fn render_bot_list_page(
    user: &users::Model,
    bots: &[users::Model],
    nonce: &csp::Nonce,
) -> Result<String> {
    let mut ctx = tera::Context::default();
    ctx.insert("user", &user);

    let bots = bots
        .iter()
        .map(|bot| {
            let token_url = web::admin::BotTokenRegenerate {
                user_id: bot.id.0.to_string(),
            }
            .to_string();
            (bot, token_url)
        })
        .collect::<Vec<_>>();
    ctx.insert("bots", &bots);

    templates::render("routes/admin/bot.html", ctx, nonce)
}

pub fn render_bot_token_page(
    user: &users::Model,
    bot: &users::Model,
    secret_token: &str,
    nonce: &csp::Nonce,
) -> Result<String> {
    let mut ctx = tera::Context::default();
    ctx.insert("user", &user);
    ctx.insert("bot", &bot);
    ctx.insert("secret_token", &secret_token);
    templates::render("routes/admin/bot-token.html", ctx, nonce)
}
