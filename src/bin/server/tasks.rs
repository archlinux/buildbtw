use anyhow::{anyhow, Context, Result};
use buildbtw::build_set_graph::calculate_packages_to_be_built;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;
use uuid::Uuid;

use crate::schedule_next_build_in_graph;
use buildbtw::{BuildNamespace, BuildSetIteration, ScheduleBuild, ScheduleBuildResult, DATABASE};

pub enum Message {
    CreateBuildNamespace(Uuid),
}

pub fn start(port: u16) -> UnboundedSender<Message> {
    println!("Starting server tasks");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::CreateBuildNamespace(namespace_id) => {
                    let namespace = {
                        let db = DATABASE.lock().await;
                        db.get(&namespace_id)
                            .unwrap_or_else(|| panic!("No build namespace for id: {namespace_id}"))
                            .clone()
                    };

                    println!("Adding namespace: {namespace:#?}");
                    println!(
                        "Graph of newest iteration will be available at: http://localhost:{port}/namespace/{}/graph",
                        namespace.id
                    );
                    if let Err(e) = create_new_build_set_iteration(&namespace).await {
                        println!("{e:?}");
                    };

                    if let Err(error) = build_namespace(namespace).await {
                        println!("{error:?}");
                    }
                }
            }
        }
    });
    sender
}

#[allow(dead_code)]
async fn new_build_set_iteration_is_needed(namespace: &BuildNamespace) -> bool {
    namespace.iterations.is_empty()
    // TODO return True if last iteration's origin_changesets are different from the current ones
    // TODO return True if git refs in last iterations package graph are outdated
    // TODO build new dependent graph and check if there are new nodes
}

async fn create_new_build_set_iteration(namespace: &BuildNamespace) -> Result<()> {
    let packages_to_be_built = calculate_packages_to_be_built(namespace).await?;

    let new_iteration = BuildSetIteration {
        id: Uuid::new_v4(),
        origin_changesets: namespace.current_origin_changesets.clone(),
        packages_to_be_built,
    };
    {
        let mut db = DATABASE.lock().await;
        let namespace_db_entry = db
            .get_mut(&namespace.id)
            .ok_or_else(|| anyhow!("Failed to access namespace in DB"))?;

        namespace_db_entry.iterations.push(new_iteration);
    }

    println!("Build set graph calculated");

    Ok(())
}

async fn build_namespace(namespace: BuildNamespace) -> Result<()> {
    // while namespace is not fully built or blocked
    loop {
        // -> schedule build
        let build = schedule_next_build_in_graph(namespace.id).await;
        match build {
            // TODO: distinguish between no pending packages and failed graph
            ScheduleBuildResult::NoPendingPackages => {
                println!("No pending packages, retry in 5 seconds");
                sleep(std::time::Duration::from_secs(5)).await;
            }
            ScheduleBuildResult::Scheduled(response) => {
                println!("Scheduled build: {:?}", response.source);
                schedule_build(response).await?;
            }
            ScheduleBuildResult::Finished => {
                println!("Graph finished");
                break;
            }
        }
    }

    Ok(())
}

async fn schedule_build(build: ScheduleBuild) -> Result<()> {
    println!(
        "Building pending package for namespace: {:?}",
        build.srcinfo.base.pkgbase
    );

    let _response = reqwest::Client::new()
        .post("http://0.0.0.0:8090/build/schedule".to_string())
        .json(&build)
        .send()
        .await
        .context("Failed to send to server")?;

    println!("Scheduled build: {:?}", build.source);
    Ok(())
}
