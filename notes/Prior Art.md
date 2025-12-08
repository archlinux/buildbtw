# Prior Art

Building on existing technology might save us from reinventing the wheel too much.
This document explores some existing full-scale solutions as well as building blocks for a new, custom service.
At the moment, using an existing solution to execute concrete chunks of build work looks like it will provide the biggest benefits.

- [Hydra](https://github.com/NixOS/hydra)
	- Only runs *after* PRs are merged
	- [There's "ofborg" for pre-merge checks](https://discourse.nixos.org/t/difference-between-ofborg-and-hydra/3235)
		- Ofborg is for untrusted code, hydra for trusted code
	- [ofborg adds labels with "rebuild counts" to merge requests](https://github.com/NixOS/nixpkgs/issues/253500). rebuilds happen after merging, in hydra
	- rebuilds run automatically
	- [Info on (re-)build efficiency](https://discourse.nixos.org/t/how-to-make-nixpkgs-more-eco-friendly-use-less-resources/20976/56)
- [Open Build Service](https://build.opensuse.org/repositories/home:sbradnick/st-sx)
	- Can package arch packages
	- Sven doesn't like the UI: Dependencies and dependents are not clearly visible
	- No way to automatically rebuild multiple packages?
	- Has Gitlab/Github integration
	- No way to group related requests (aka todos), e.g. for a big rebuild
	- Quite a complex setup with many moving parts
	- [Can automatically rebuild packages](https://openbuildservice.org/help/manuals/obs-user-guide/cha.obs.build_scheduling_and_dispatching)
	- Also seems to do releases by moving packages between repos ("projects" in this case)
- [Koji](https://fedoraproject.org/wiki/Koji)
	- [Info on Rebuilds](https://docs.fedoraproject.org/en-US/package-maintainers/Package_Update_Guide/#updating_inter_dependent_packages)
	- [Bodhi](https://fedoraproject.org/wiki/Bodhi) releases packages
- Void Buildbot
	- [Waterfall Display](https://build.voidlinux.org/waterfall)
	- [bulk rebuilds](https://github.com/void-linux/xbps-bulk)
	- [Possibly no atomicity for publishing package sets?](https://docs.voidlinux.org/xbps/troubleshooting/common-issues.html#shlib-errors)
	- doc links on homepage are broken, and [there are some worrying issues](https://github.com/buildbot/buildbot/issues/7836)
- https://github.com/felixonmars/archlinux-futils/blob/master/gorebuild
	- Felix breaks cycles by removing makedepends & checkdepends automatically
- https://github.com/alucryd/archbuild
- https://gitlab.archlinux.org/foxboron/archlinux-buildbot
- https://gitlab.com/herecura/templates/gitlab-ci
- https://osg-htc.org/technology/software/koji-mass-rebuilds/
- https://github.com/foutrelis/arch-rebuilds
- [ALHP](https://somegit.dev/ALHP/ALHP.GO)
- Serpent OS
	- They don't have CI for a PR-based workflow yet
    - Build process builds a snapshot of the whole world
    - They have three services: coordinator, repo manager, builder
    - heavily lean on build manifests for recording build inputs & potential outputs, similar to .SRCINFO. See https://github.com/serpent-os/recipes/blob/main/z/zlib/manifest.x86_64.jsonc for an example
    - Cycles are already prevented while constructing the dep graph, each added edge checks whether it would introduce a cycle
        - This doesn't solve most situations but at least allows for a topological sort
    - They're working out a concept for building in intermediate, isolated stages before publishing to the main repo, similar to us, but there seems to be nothing concrete yet
- [Buildbot](https://buildbot.net/) ([docs](https://docs.buildbot.net/current/))
    - Configured in Python
    - Very flexible
    - Python updates can be problematic
- [Dagger](https://dagger.io/)
    - Configured in Python, TypeScript or Go
    - Very flexible
    - Trying very hard to be a platform (See "daggerverse"). This could get in our way
    - no self-hosting docs?

## Buildbot as a buildbtw worker

### Advantages

- Existing Web UI
- Authorization & Authentication
- Worker infrastructure management
- Mature, battle tested codebase
- Works for at least one other distribution (with different approach to building)

### Disadvantages

- REST API is severely limited, we would need to extend buildbot using the [python data API](https://docs.buildbot.net/current/developer/data.html#), or use other workarounds to dynamically dispatch build jobs
    - There are few examples of this, and it is not well documented
- It is not customary to have dynamic build sets, and it would be very hard to dynamically dispatch a single build for each package
    - no graph display
- Void uses buildbot, but doesn't have per-package builds: https://build.voidlinux.org/#/
    - They have a [pile of workarounds](https://github.com/void-linux/xbps-bulk/blob/master/configure) to run multiple dynamic builds inside a single buildbot build step
- Separate system from buildbtw, adding friction and complexity
- Different language from the rest of the buildbtw codebase
- We would need to customize buildbot very deeply, and we don't know how well that customization would work with future buildbot updates
