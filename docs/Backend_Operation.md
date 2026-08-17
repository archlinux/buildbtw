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
The container will write the public key to `/etc/ssh/known_hosts` on startup.
You can retrieve this key using `ssh-keyscan -t ed25519 gitlab.archlinux.org`.
