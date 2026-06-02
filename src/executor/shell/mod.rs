use rust_embed::Embed;

#[expect(
    missing_debug_implementations,
    reason = "empty structs don't need debug"
)]
#[derive(Embed)]
#[folder = "src/executor/shell"]
#[include = "*.sh"]
pub struct ShellScripts;
