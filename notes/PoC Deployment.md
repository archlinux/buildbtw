# Proof-of-concept deployment

We're running an evaluation deployment on the `buildbtw-dev` host. 

## Overview

- Runs in a rootless podman container
- Accessible via SSH port forwarding only

## Access the service

After gaining access to the `buildbtw-dev` host, forward the buildbtw server port to your local machine:

```sh
ssh -L 8080:localhost:8080 buildbtw-dev
```

Then open [http://localhost:8080] in your browser.

With the default `.env` file, the buildbtw client should be able to connect to the server this way as well.

## Deploying a new version

```sh
sudo -u buildbtw -i podman pull registry.archlinux.org/archlinux/buildbtw:poc-server-latest
sudo -u buildbtw -i \
    podman run \
        --env-file /srv/buildbtw/env \
        --restart always \
        --replace --name server \
        --volume /srv/buildbtw/data:/app/data \
        --publish 127.0.0.1:8080:8080 \
        registry.archlinux.org/archlinux/buildbtw:poc-server-latest \
        run
```

## Initial Setup

Commands used to set up the deployment:

```sh
sudo pacman -Syu podman crun
sudo systemctl enable podman-restart.service
# Check that overlay is enabled
podman info | grep -i overlay
sudo useradd -U -d /srv/buildbtw buildbtw
sudo loginctl enable-linger buildbtw
sudo -u buildbtw -i mkdir /srv/buildbtw/data
# Look at .env.example for configuring the server
# Make sure to set the database location to /app/data
sudo -u buildbtw -i vim /srv/buildbtw/env
sudo chmod go-rwx /srv/buildbtw/env
```

Proceed to "Deploying a new version".
