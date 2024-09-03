run-server *args:
    cargo run --bin server -- run {{ args }}

watch-server *args:
    systemfd --no-pid -s http::8080 -- cargo watch -- just run-server {{args}}

run-client *args:
    cargo run --bin client -- {{ args }}

watch-client *args:
    cargo watch -- just run-client {{args}}

warmup-server *args:
    cargo run --bin server -- warmup {{ args }}
