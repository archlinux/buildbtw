FROM registry.archlinux.org/archlinux/archlinux-docker:base-master
RUN pacman -Syu --noconfirm libgit2 && pacman -Scc --noconfirm
COPY --chmod=755 target/release/backend /app/
COPY --parents common-style/ assets/ templates/ /app/
WORKDIR /app
ENTRYPOINT ["/app/backend"]
