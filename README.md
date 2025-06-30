# buildbtw software collection

## Projects

This repo contains a bunch of software maintained by the buildbtw team.
Check the respective directories for their READMEs.

- [buildbtw-poc](/buildbtw-poc) - the proof of concept buildbtw implementation
- [arch-pkg-repo-updater](/arch-pkg-repo-updater) - a tool to sync package repositories

## Commands

There are a bunch of commands you can run at this level:

- `just ci-dev` to check whether the repo as a whole would pass CI
- `just licenses` to check license compliance
    - Requirement: `reuse` (`pacman -S reuse`)
