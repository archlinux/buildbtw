use std::collections::HashMap;

use tokio::sync::mpsc::UnboundedSender;

use crate::BuildNamespace;

pub enum Message {
    CreateBuildNamespace(BuildNamespace),
}

pub fn start() -> UnboundedSender<Message> {
    let mut namespaces = HashMap::new();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::CreateBuildNamespace(namespace) => {
                    println!("Adding namespace: {namespace:#?}");
                    namespaces.insert(namespace.id, namespace);
                }
            }
        }
    });
    sender
}

async fn new_build_set_iteration_is_needed(namespace: &BuildNamespace) -> bool {
    namespace.iterations.is_empty()
    // TODO return True if last iteration's origin_changesets are different from the current ones
    // TODO return True if git refs in last iterations package graph are outdated
    // TODO build new dependent graph and check if there are new nodes
}

async fn create_new_build_set_iteration() {}
