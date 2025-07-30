# buildbtw software collection

## Projects

This repo contains a bunch of software maintained by the buildbtw team.
Check the respective directories for their READMEs.

- [buildbtw-poc](/buildbtw-poc) - the proof of concept buildbtw implementation
- [arch-pkg-repo-updater](/arch-pkg-repo-updater) - a tool to sync package repositories

## Documentation

- [Architecture Overview](notes/Architecture_Overview.md)
- [PoC User Guide](notes/PoC_User_Guide.md)
- [PoC Deployment](notes/PoC_Deployment.md)

## Development

- Install Rust. It's recommended to work with the stable toolchain by default, but to format the code, you'll need the nightly toolchain as well. With rustup:
```
rustup install stable nightly
rustup default stable
```

## Commands

There are a bunch of commands you can run at this level:

- `just ci-dev` to check whether the repo as a whole would pass CI
- `just licenses` to check license compliance
    - Requirement: `reuse` (`pacman -S reuse`)
