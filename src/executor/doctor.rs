use color_eyre::Result;
use yansi::Paint;

use crate::{
    api_client::{ApiClient, user::current},
    executor::config::DoctorConfig,
};

/// Check that the executor is ready to operate
pub async fn doctor(config: DoctorConfig) -> Result<()> {
    let ok = format!("[ {} ]", "OK".bold().green());
    let fail = format!("[{}]", "FAIL".bold().red());

    // Find vmexec
    let vmexec_path = which::which_global("vmexec").ok();
    let vmexec = "vmexec".bold();
    if let Some(vmexec_path) = vmexec_path {
        println!(
            "{ok} {vmexec}: Found vmexec at {}",
            vmexec_path.to_string_lossy().underline()
        );
    } else {
        println!("{fail} {vmexec}: Didn't find vmexec");
    }

    let login = "login".bold();
    if let Some(auth) = config.auth {
        let api_client =
            ApiClient::with_token(auth.api_server_url.clone(), auth.api_token.expose_secret())?;
        if let Some(user) = current(&api_client).await? {
            println!("{ok} {login}: Logged in as {}", user.username);
        } else {
            println!("{fail} {login}: Couldn't log in with provided token");
        }
    } else {
        println!("{fail} {login}: No login token found");
    }

    Ok(())
}
