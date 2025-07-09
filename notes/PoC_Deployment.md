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
sudo -u buildbtw -i /srv/buildbtw/deploy-server.sh
```

## Initial Setup

Commands used to set up the deployment:

```sh
sudo pacman -Syu podman crun
# Check that overlay is enabled
podman info | grep -i overlay
sudo useradd -U -d /srv/buildbtw buildbtw
sudo loginctl enable-linger buildbtw
sudo systemctl --user -M buildbtw@ enable podman-restart.service
sudo -u buildbtw -i mkdir /srv/buildbtw/data
# Look at .env.example for configuring the server
# Make sure to set the database location to /app/data
sudo -u buildbtw -i vim /srv/buildbtw/env
sudo chmod go-rwx /srv/buildbtw/env
```

Add an SSH key and add it in base64-encoded form to the Gitlab CI as a secret with the name "BUILDBTW_SSH_PRIVATE_KEY". Add the IPv4 address of the server as a secret with the name "BUILDBTW_SERVER_IPV4".

Now, let the gitlab `deploy` CI job run to automatically create the deploy script on the server and execute it.
