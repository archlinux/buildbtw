# Prior Art

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