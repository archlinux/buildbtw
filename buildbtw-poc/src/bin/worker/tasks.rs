use tokio::sync::mpsc::UnboundedSender;

use crate::{set_build_status, upload_packages};
use buildbtw_poc::{build_package::build_package, PackageBuildStatus, ScheduleBuild};

pub enum Message {
    BuildPackage(ScheduleBuild),
}

pub fn start(modify_gpg_keyring: bool) -> UnboundedSender<Message> {
    tracing::info!("Starting worker tasks");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::BuildPackage(schedule) => {
                    tracing::info!("🕑 Building package {:?}", schedule.source.0);
                    let mut result_status = build_package(&schedule, modify_gpg_keyring).await;

                    tracing::info!(
                        "build result for {:?}: {result_status:?}",
                        schedule.source.0
                    );

                    // TODO we might want to guarantee some kind of transactionality
                    // for the upload + status update operations
                    if let Err(err) = upload_packages(&schedule).await {
                        result_status = PackageBuildStatus::Failed;
                        tracing::error!(
                            "Uploading package failed (marking build as failed): {err:?}"
                        );
                    }

                    // TODO: retry with exponential backoff
                    if let Err(err) = set_build_status(result_status, &schedule).await {
                        tracing::error!("❌ Failed to set build status: {err:?}");
                    }
                }
            }
        }
    });
    sender
}
