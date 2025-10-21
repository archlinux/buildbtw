FROM registry.archlinux.org/archlinux/archlinux-docker:base-master
COPY --chmod=755 backend /app/
ENTRYPOINT ["/app/backend"]
