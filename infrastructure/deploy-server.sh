#!/bin/bash
set -o nounset -o errexit -o pipefail -o xtrace

echo "Pulling image..."
podman pull registry.archlinux.org/archlinux/buildbtw:poc-server-latest
echo "Starting container..."
podman run \
    --env-file /srv/buildbtw/env \
    --restart always \
    --detach \
    --replace --name server \
    --volume /srv/buildbtw/data:/app/data \
    --volume /srv/buildbtw/.ssh/id_ed25519:/ssh_id:ro \
    --volume /srv/buildbtw/.ssh/known_hosts:/root/.ssh/known_hosts:ro \
    --publish 127.0.0.1:8080:8080 \
    registry.archlinux.org/archlinux/buildbtw:poc-server-latest