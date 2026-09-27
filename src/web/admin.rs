//! Routes and parameters for admins
use axum_extra::routing::TypedPath;
use serde::Deserialize;

/// List bot users and create new ones.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/admin/bot")]
pub struct BotList {}

/// Create a new bot user along with its token.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/admin/bot")]
pub struct BotCreate {}

/// Regenerate the token of an existing bot user.
///
/// Note: The previous token will stop working.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/admin/bot/{user_id}/token")]
pub struct BotTokenRegenerate {
    /// Id of the bot user
    pub user_id: String,
}
