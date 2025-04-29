FROM docker.io/library/archlinux:base
COPY --chmod=755 server /app/buildbtw-server
RUN pacman -Syu --noconfirm openssh libgit2
CMD ["ssh-agent", "bash", "-c", "ssh-add /ssh_id; /app/buildbtw-server run"]
