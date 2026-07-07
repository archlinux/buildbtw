use camino::Utf8Path;
use color_eyre::{Result, eyre::OptionExt};
use tera::Tera;
use time::{OffsetDateTime, format_description};
use tokio::sync::OnceCell;

pub mod account;
pub mod index;

/// Path to the templates directory.
const TEMPLATE_PATH: &str = "templates";

/// Global Tera instance initialized once at startup.
/// This is done with a once-cell instead of lazily so Tera eagerly compiles all the
/// templates during backend startup which allows to catch fatal issues early on.
static TERA: OnceCell<Tera> = OnceCell::const_new();

/// Initialize the template module.
pub fn initialize(root: &Utf8Path) -> Result<()> {
    Ok(TERA.set(initialize_tera(root)?)?)
}

fn initialize_tera(root: &Utf8Path) -> Result<Tera> {
    let mut tera = Tera::new(format!("{root}/{TEMPLATE_PATH}/**/*").as_str())?;
    tera.register_filter("datetime", format_datetime);
    Ok(tera)
}

fn format_datetime(
    value: &tera::Value,
    _: &std::collections::HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let datetime: OffsetDateTime = serde_json::from_value(value.clone())?;
    let format = format_description::parse_borrowed::<3>(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]",
    ).map_err(|e| tera::Error::msg(format!("failed to create format description: {e}")))?;
    Ok(tera::Value::String(datetime.format(&format).map_err(
        |e| tera::Error::msg(format!("failed to format datetime: {e}")),
    )?))
}

/// Helper to access the pre-compiled static Tera cell and render the passed template.
fn render(template_name: &str, mut context: tera::Context) -> Result<String> {
    let tera = TERA
        .get()
        .ok_or_eyre("Template engine is not initialized")?;
    context.insert("index_url", &crate::web::index::Index {}.to_string());
    context.insert("login_url", &crate::web::oidc::StartLogin {}.to_string());
    context.insert("logout_url", &crate::web::account::Logout {}.to_string());
    context.insert("account_url", &crate::web::account::Overview {}.to_string());
    context.insert(
        "session_list_url",
        &crate::web::account::SessionList {}.to_string(),
    );
    context.insert(
        "cli_session_url",
        &crate::web::account::CliSessionLanding {}.to_string(),
    );
    Ok(tera.render(template_name, &context)?)
}
