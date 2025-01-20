use tokio::sync::mpsc::UnboundedSender;

use crate::set_build_status;
use buildbtw::{build_package::build_package, ScheduleBuild};

pub enum Message {
    BuildPackage(ScheduleBuild),
}

pub fn start() -> UnboundedSender<Message> {
    tracing::info!("Starting worker tasks");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::BuildPackage(schedule) => {
                    tracing::info!("🕑 Building package {:?}", schedule.srcinfo.base.pkgbase);
                    let result_status = build_package(&schedule).await;
                    tracing::info!("✅ built package {:?}", schedule.srcinfo.base.pkgbase);

                    // TODO: exponential backoff
                    if let Err(err) = set_build_status(
                        schedule.namespace,
                        schedule.iteration,
                        schedule.source.0,
                        result_status,
                    )
                    .await
                    {
                        tracing::info!("❌ Failed to set build status: {err}");
                    }
                }
            }
        }
    });
    sender
}
