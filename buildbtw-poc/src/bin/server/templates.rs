use anyhow::{Context, Result};

const TEMPLATES: &[&str] = &[
    "layout",
    "show_build_namespace",
    "render_build_namespace_graph",
    "list_build_namespaces",
];

#[derive(rust_embed::Embed)]
#[folder = "templates"]
pub struct Templates;

pub fn add_to_jinja_env(jinja_env: &mut minijinja::Environment) -> Result<()> {
    for template_name in TEMPLATES {
        let contents = String::from_utf8(
            Templates::get(&format!("{template_name}.jinja"))
                .context(template_name)
                .context("Could not find template")?
                .data
                .to_vec(),
        )?;
        jinja_env.add_template_owned(*template_name, contents)?;
    }
    Ok(())
}
