run-server *args:
    cargo run --bin server -- run {{ args }}

watch-server *args:
    systemfd --no-pid -s http::8080 -- cargo watch -w src -w templates -w Cargo.toml -- just run-server {{ args }}

run-client *args:
    cargo run --bin client -- {{ args }}

watch-client *args:
    cargo watch -w src -w templates -w Cargo.toml -- just run-client {{ args }}

warmup-server *args:
    cargo run --bin server -- warmup {{ args }}

clean:
    rm -rf source_repos

test *args:
    cargo test {{ args }}

watch-test *args:
    cargo watch -w src -w templates -w Cargo.toml -- just test {{ args }}
