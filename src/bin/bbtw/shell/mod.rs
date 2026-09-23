use rust_embed::Embed;

#[derive(Embed)]
#[folder = "src/bin/bbtw/shell"]
#[include = "*.sh"]
pub struct ShellScripts;
