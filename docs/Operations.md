# Operating the buildbtw backend

A guide for deploying and administering the buildbtw backend server.

The preferred mode of deployment is as a container. Containers are published to [the project's container registry](https://gitlab.archlinux.org/archlinux/buildbtw/container_registry/22). For production, you'll need to find the tag for the specific release number you want to deploy.

## Configuration

Refer to the output of `buildbtw-backend --help` for up-to-date information on all configuration options.

The backend can be configured via command-line flags or environment variables. Using environment variables is recommended.

### OIDC

Configure your OIDC-compliant issuer using the `BUILDBTW_OIDC_` options - you'll need an URL to reach your issuer at, a client ID and a client secret.

By default, OIDC users can log in, but to do more than that, you'll need to assign roles to them.
There's a configuration option for each role, e.g. `BUILDBTW_OIDC_PACKAGE_MAINTAINER_GROUPS`.
This is a comma-separated list of group names.
When a user logs in, buildbtw also receives the groups they are part of, as configured in your OIDC provider.
Using buildbtw's options, you can now map multiple of these OIDC groups to a specific buildbtw role.
Buildbtw will periodically re-fetch the groups from the OIDC provider and update assigned roles accordingly.

### Using the container

When running the backend via `podman`, it's important to set `BUILDBTW_GITLAB_SSH_HOST_KEY` to the SSH public key of your GitLab instance.
For instance, for Arch Linux, this is `gitlab.archlinux.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICjT2SuA0k/xc5Cbyp+eBY5uN3bRL2K7GdpNtltOK6vy`.
The container will write the public key to `/etc/ssh/ssh_known_hosts` on startup.
You can retrieve this key using `ssh-keyscan -t ed25519 gitlab.archlinux.org`.

We also expect an SSH private key for cloning the repositories from GitLab to get mounted to `/etc/ssh/id_ed25519` inside the container.
This key is shared amongst all containers and should be mounted read-only.
The container launches an agent that looks for a key at that location. If the key isn't present, the container will not launch.

### Reverse Proxy

When running the backend behind a reverse proxy, you need to make sure that `Host` and `Origin` are passed through unchanged in order for
the CSRF protection to work as intended.
Refer to [this article](https://words.filippo.io/csrf/#protecting-against-csrf-in-2025) for details.

# Operating the buildbtw GitLab Custom Executor

Make sure that your backend is deployed before trying to deploy the executor.

## Bot token
In order for the cutom executor to be able to talk to the backend, we'll need a bot token.
Log in as admin to the backend and create a new bot token at `https://buildbtw.example.com/admin/bot`.
Note down that token.

## Installation

Compile the custom executor and install the resulting binary to `/usr/local/bin/buildbtw-execturor`.

## Runner Config

Register a new GitLab Runner like you'd normally do. We then need to give it some custom config in its `/etc/gitlab-runner/config.toml`:
```
[[runners]]
  name = "buildbtw-runner"
  url = "https://gitlab.archlinux.org"
  token = "your-gitlab-runner-token"
  executor = "custom"
  builds_dir = "/builds"
  cache_dir = "/cache"
  environment = [
    "RUST_BACKTRACE=1",
    "BUILDBTW_SERVER_URL=https://buildbtw.example.com/admin/bot",
    "BUILDBTW_EXECUTOR_TOKEN=your-bot-token-from-earlier",
  ]
  [runners.custom]
    config_exec = "/usr/local/bin/buildbtw-executor"
    config_args = [ "gitlab", "config" ]

    prepare_exec = "/usr/local/bin/buildbtw-executor"
    prepare_args = [ "gitlab", "prepare" ]

    run_exec = "/usr/local/bin/buildbtw-executor"
    run_args = [ "gitlab", "run" ]

    cleanup_exec = "/usr/local/bin/buildbtw-executor"
    cleanup_args = [ "gitlab", "cleanup" ]
```

Restart the runner. Your GitLab Runner should now be ready to go with the buildbtw GitLab Custom Executor.
