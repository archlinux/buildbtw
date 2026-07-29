# buildbtw project management meeting log

<!-- Prepend new meetings here -->

[issue board](https://gitlab.archlinux.org/archlinux/buildbtw/-/boards/24162?milestone_title=Started)

## 2026-07-28

- [Split out the web UI part from this issue](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/43)
- Rename `bbtw close` to `bbtw stop` based on user feedback
- In the future, cancel already running builds when stopping

## 2026-07-21

### Topics

- Status updates
- Roadmap to Summit
- Boundary Diagram
- Schedule meatspace worktime
- Deployment fucked

### bbtw cancel

- Name the command `bbtw close` instead, because it doesn't cancel running builds

### Roadmap

- [here](https://excalidraw.com/#room=9ada9c07726b1b944a35,yjWvU_nZ6URSY0qPXg8EGg) and in [this MR](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/237)

### SQLite transactions

- Use `IMMEDIATE` transactions for any transaction involving writes
- Use multiple short transactions rather than one long-running transaction

## In-person working

- 2026-07-28 10:00

## 2026-06-23

- Invite 1-2 people for very early testing
- Do focus days / a focus week in meatspace late july / early august

## 2026-06-16

- Delete worker module since the executor is now embedded in the backend server
- Bump priority of [#271](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/271) because we keep needing it for local debugging
- For local executor, mount a temporary dir in the VM and move the artifacts into permanent storage only after build succeeded

## 2026-06-09

- secure runner 1 will run only buildbtw builds on protected branches
- staging runner will run buildbtw builds for forks, instance wide

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
