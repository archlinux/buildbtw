FROM registry.archlinux.org/archlinux/archlinux-docker:base-master
RUN pacman -Syu --noconfirm libgit2 && pacman -Scc --noconfirm
COPY --chmod=755 target/release/buildbtw-backend /app/
COPY --parents common-style/ assets/ templates/ /app/
WORKDIR /app

# We inject the GitLab host key into /etc/ssh/known_hosts.
# Sadly, this is the only way we can reliably get git's SSH transport to pick the right key.
# libgit2 doesn't provide a programmatic mechanism for us to request a specific host key.
# See also https://github.com/libgit2/libgit2/pull/6449/changes/63b083e5d8ecea0750eb5767018b1cdad94c2382
ENTRYPOINT ["sh", "-c", "echo \"$BUILDBTW_GITLAB_SSH_HOST_KEY\" > /etc/ssh/known_hosts && exec /app/buildbtw-backend \"$@\"", "sh"]
