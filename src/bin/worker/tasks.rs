use tokio::time::sleep;
use tokio::sync::mpsc::UnboundedSender;

use crate::set_build_status;
use buildbtw::{PackageBuildStatus, ScheduleBuild};

pub enum Message {
    BuildPackage(ScheduleBuild),
}

pub fn start(port: u16) -> UnboundedSender<Message> {
    println!("Starting worker tasks");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::BuildPackage(schedule) => {
                    println!("🕑 Building package {:?}", schedule);
                    sleep(std::time::Duration::from_secs(3)).await;
                    println!("✅ building package {:?}", schedule);

                    // Failure is not an option :P
                    let result = PackageBuildStatus::Built;

                    // TODO: exponential backoff
                    if let Err(err) = set_build_status(schedule.namespace, schedule.iteration, schedule.source.0, result).await {
                        println!("❌ Failed to set build status: {err}");
                    }
                }
            }
        }
    });
    sender
}