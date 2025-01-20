fn main() {
    println!("cargo::rerun-if-changed=src/gitlab/gitlab_schema.json");
}
