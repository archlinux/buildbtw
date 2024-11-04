# Buildbot as a buildbtw worker

## Advantages

- Existing Web UI
- Authorization & Authentication 
- Worker infrastructure management
- Mature, battle tested codebase
- Works for at least one other distribution (with different approach to building)

## Disadvantages

- REST API is severely limited, we would need to extend buildbot using the python data API, or use other workarounds to dynamically dispatch build jobs
    - There are few examples of this, and it is not well documented
- It is not customary to have dynamic build sets, and it would be very hard to create a single build for each package
    - no graph display
- Void uses buildbot, but doesn't have per-package builds: https://build.voidlinux.org/#/
- Separate system from buildbtw, adding friction and complexity
- Different language from the rest of the buildbtw codebase
- We would need to customize buildbot very deeply, and we don't know how well that customization would work with future buildbot updates