# Arch Summit buildbtw Discussion

🫂 **Attendees**: Sven, Levente, Rafael
⏰ **Date/Time**: 2024-11-23 16:00

## GitOps / Gitlab

- building locally and remotely is the same from buildbtw's view
- having a namespace with 6k packages in the release queue will cause frequent failures and might never be released 
    - we might need to add locks
    - pkgctl could emit warnings
    - new iterations can reuse artifacts from previous iterations
- it's not sure whether we want to interrupt outdated iterations
- gitlab issues might not work well for large rebuilds
- having many issues, branches, MRs and pipelines might be problematic, especially in terms of API access
    - we need to test this
    - what if gitlab crashes while we're halfway through creating merge requests for a rebuild?
        - buildbtw always reconciliates external state
        - add retries at many points
- Rebasing source branches
    - When main branch is updated, will branches be automatically rebased by buildbtw?
    - Buildbtw will evaluate if it can rebase branches safely. If so, it will rebase
    - In cases where conflicts arise, buildbtw will not automatically rebase
    - Should we try to encourage linear git history in some way?

## Multi-arch support

- We try to avoid cross-compiling, getting native runners
- no great hardware for RISC-V atm
    - it's gonna be some time before RISC-V becomes usable in day-to-day operations

## MVP: Single-server or Cluster?

- Single-server
- Gitlab runner infrastructure *should* make this easy
- gonna constrain build resources using VMs

## Why not containerization?

- Security implications
- could be made to work with "trusted" builds
- using VMs everywhere is still more secure

## Artifact retention & storage

- Logs will still be stored inside buildbtw, even if they're also available in gitlab
- Remove namespaces or not?
    - Kill them!
        - Many iterations consume lots of disk space
    - Don't kill them!
        - Might be linked from discussions
        - Giovanni had another point about this
    - Compromise: Keep release queue logs on success and discard all other artifacts

## Release strategies

 - Manual: Allow users to select packages to release from a list
- Semi-Automatic
    - Have a reasonable default heuristic that can be disabled or configured
    - need to be able to manually override release strategy while build is running
    - this can make the tool harder to maintain
- Fully automatic
    - Impossible to detect
- Just release everything that's built: ❌
    - strains user & mirror bandwidth
- Buildbtw will always print a summary of what is going to be released before users promote a namespace

## Signing

- Signstar signing request will be sent outside the builder VM
- First MVP won't sign packages on-server, packagers will sign locally like before

## Long-term maintenance & additional contributors

Time out :<

# Action items / ToDo's

- test whether gitlab can handle a large amount of MRs (buildbtw team)