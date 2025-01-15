fn main() {
    println!("cargo::rerun-if-changed=src/gitlab/gitlab_schema.json");
    // TODO: Somehow, this does nothing
    println!("cargo::rustc-cfg=tokio_unstable");
}
