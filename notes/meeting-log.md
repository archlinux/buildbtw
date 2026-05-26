# buildbtw project management meeting log

<!-- Prepend new meetings here -->

## 2026-05-26

- Focus on CLI first: build the web UI later on, when more of the high-prio tasks (scheduling, building, deployment) are done
    - Also split out Web UI tasks into separate tickets, atm they are lumped in with CLI tasks
- In integration tests, when we have to configure the system, generally try to avoid conditional compilation, and instead rely on explicit configuration that is passed in.
