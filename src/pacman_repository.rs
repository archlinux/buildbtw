//! Pacman package repository manager.
//!
//! The repo-manager handles pacman repository operations for the build
//! system, creating repository databases using `repo-add` and organizing
//! package artifacts in iteration-specific repositories.
//!
//! Repositories are stored in buildbtw's configured artifact directory and served
//! for dependency resolution during subsequent builds in the same iteration.
