FROM registry.archlinux.org/archlinux/archlinux-docker:base-master
COPY --chmod=755 target/release/backend /app/
COPY --parents common-style/ assets/ templates/ /app/
WORKDIR /app
ENTRYPOINT ["/app/backend"]
