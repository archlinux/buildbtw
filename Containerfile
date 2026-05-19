FROM registry.archlinux.org/archlinux/archlinux-docker:base-master
RUN pacman -Syu --noconfirm libgit2 && pacman -Scc --noconfirm
COPY --chmod=755 target/release/buildbtw-backend /app/

# The executor is not actually being used as such in this image.
# We're merely using the OCI as a delivery mechanism for the binary as we can't use
# a podman container as a GitLab executor entrypoint.
# See also https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/199
COPY --chmod=755 target/release/buildbtw-executor /app/

COPY --parents common-style/ assets/ templates/ /app/
WORKDIR /app
ENTRYPOINT ["/app/buildbtw-backend"]
