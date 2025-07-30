# Architecture Overview

## Components

buildbtw follows a client-server architecture with four primary components:

1. **Server** (`buildbtw-server`): Central web service providing REST API, web interface, and orchestration logic
2. **Client** (`buildbtw-client`): CLI tool for dispatching build requests and inspecting system state
3. **Worker** (`buildbtw-worker`): Local build executor that processes packages using `pkgctl`
4. **GitLab Custom Executor**: Integration layer for GitLab CI pipelines

## Server Functionality

**Build Set Graph System**: The heart of buildbtw uses the `petgraph` crate to model package dependencies as directed graphs. It computes which packages need rebuilding when dependencies change, creates a graph for each architecture to build, and manages build status propagation (Blocked, Pending, Scheduled, Building, Built, Failed).

**Build Namespace Management**: Each isolated namespace contains multiple iterations that track origin changesets, build graphs per architecture, and iteration history with diff tracking.

**Source Repository Integration**: Integrates GitLab APIs to monitor package source repositories, trigger automated builds on changes, and handle git operations for package source management.

**Database Layer**: Uses SQLite with `sqlx` for persistence.

**Build Execution Infrastructure**: buildbtw uses a custom GitLab executor (`infrastructure/buildbtw-executor.sh`) that orchestrates isolated package builds within VMs using `vmexec`. Builds run in secure VM environments with user-mode networking, where the host buildbtw server is accessible at `10.0.2.2`. The VM script (`build-inside-vm.sh`) configures pacman repositories, imports GPG keys, and executes `pkgctl build` as a non-root user, with built packages uploaded back to the server via HTTP POST.

**Package Repository Management**: buildbtw maintains isolated pacman repositories for each build namespace iteration using the `pacman_repo` module. Repository databases are stored in `./data/repos/{namespace}_{iteration}/os/{architecture}/` and managed using standard `repo-add` commands. Each repository serves as a custom pacman source during builds, allowing packages from earlier builds in the same namespace to be available as dependencies for subsequent builds.

**Background Job Processing**: The server runs multiple background task loops for different responsibilities: GitLab source repository change monitoring, CI configuration updates, and build scheduling/status updates. Tasks communicate via `tokio::sync::mpsc` channels, with the server orchestrating build dispatch either to local workers via HTTP API or to GitLab pipelines via GitLab API calls. The worker processes builds asynchronously, handling package compilation and artifact upload.
