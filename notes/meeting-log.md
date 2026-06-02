# buildbtw project management meeting log

<!-- Prepend new meetings here -->

## 2026-06-02

- Deploy review & staging executors to runner2
- No runner for review environments
- We want the production deployment ready for the summit
- For smaller buids & early feedback, staging is sufficient
- [move oidc client secret cli argument to external-secrets](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/268)

### Review Velocity

- try to avoid nitpicks
- in subsequent review rounds, only review new code
- push small fixes to MRs of others, if it's obvious that both parties would agree to the change
- what to do when blocked?
    - work on other stuff if possible
- presence times
    - schedule semi-spontaneous discussions a day or two earlier
    - **extended meeting on tuesdays 16:00 - 18:00**
        - collect discussion topics in advance
        - do collective reviews, sparring, pair programming...
- do reviews more proactively
- request reviews directly

## 2026-05-26

- Focus on CLI first: build the web UI later on, when more of the high-prio tasks (scheduling, building, deployment) are done
    - Also split out Web UI tasks into separate tickets, atm they are lumped in with CLI tasks
- In integration tests, when we have to configure the system, generally try to avoid conditional compilation, and instead rely on explicit configuration that is passed in.
