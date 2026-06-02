FROM registry.archlinux.org/archlinux/archlinux-docker:base-master
RUN pacman -Syu --noconfirm libgit2 && pacman -Scc --noconfirm
COPY --chmod=755 target/release/buildbtw-backend /app/
COPY --parents common-style/ assets/ templates/ /app/
WORKDIR /app
ENTRYPOINT ["/app/buildbtw-backend"]
