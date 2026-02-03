use rust_embed::Embed;

#[derive(Embed)]
#[folder = "src/bin/executor/shell"]
#[include = "*.sh"]
pub struct ShellScripts;
