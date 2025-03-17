FROM docker.io/library/archlinux:base
COPY --chmod=755 server /app/buildbtw-server
ENTRYPOINT ["/app/buildbtw-server"]
