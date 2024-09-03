run-server *args: 
    cargo run --bin server -- run {{ args }}

watch-server *args:
    cargo-watch -- just run-server {{args}}

run-client *args: 
    cargo run --bin client -- {{ args }}

watch-client *args:
    cargo-watch -- just run-client {{args}}