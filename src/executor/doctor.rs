use color_eyre::Result;
use color_eyre::eyre::bail;
use yansi::Paint;

use crate::{api_client::user::current, executor::config::DoctorConfig};

/// Check that the executor is ready to operate
pub async fn doctor(config: DoctorConfig) -> Result<()> {
    let ok = format!("[  {}  ]", "OK".bold().green());
    let fail = format!("[ {} ]", "FAIL".bold().red());

    let mut failed = false;

    let login = "login".bold();
    if let Some(auth) = config.api_config {
        let api_client = auth.build_api_client()?;
        if let Some(user) = current(&api_client).await? {
            eprintln!("{ok} {login}: Logged in as {}", user.username);
        } else {
            failed = true;
            eprintln!("{fail} {login}: Couldn't log in with provided token");
        }
    } else {
        failed = true;
        eprintln!("{fail} {login}: No login token found");
    }

    if failed {
        bail!("doctor checks failed");
    }

    Ok(())
}
