fn main() {
    println!("cargo::rerun-if-changed=src/gitlab_schema.json");
    println!("cargo::rerun-if-changed=src/*.graphql");
}
