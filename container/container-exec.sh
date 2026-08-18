#!/bin/bash
set -euo pipefail

# We inject the GitLab host key into /etc/ssh/ssh_known_hosts.
# Sadly, this is the only way we can reliably get git's SSH transport to pick the right key.
# libgit2 doesn't provide a programmatic mechanism for us to request a specific host key.
# See also https://github.com/libgit2/libgit2/pull/6449/changes/63b083e5d8ecea0750eb5767018b1cdad94c2382
echo "$BUILDBTW_GITLAB_SSH_HOST_KEY" > /etc/ssh/ssh_known_hosts

gitlab_domain=$(echo "${BUILDBTW_GITLAB_DOMAIN}" | sed -E 's|^https?://||')
if ! ssh-keygen -f /etc/ssh/ssh_known_hosts -F "${gitlab_domain}"; then
    echo "ERROR: Cannot connect to GitLab instance ${gitlab_domain} because SSH host key is mismatching"
    exit 1
fi

# Note: this will silently skip invalid public keys if the file contains any
# valid public key. As long as we only write one public key, that's fine.
if ! ssh-keygen -l -f /etc/ssh/ssh_known_hosts; then
    echo 'ERROR: BUILDBTW_GITLAB_SSH_HOST_KEY is not a valid ssh_known_hosts entry' >&2
    exit 1
fi

if [[ ! -f /etc/ssh/id_ed25519 ]]; then
    echo 'ERROR: /etc/ssh/id_ed25519 must exist' >&2
    exit 1
fi
eval "$(ssh-agent -s)"
if ! ssh-add /etc/ssh/id_ed25519; then
    echo 'ERROR: ssh-add failed' >&2
    exit 1
fi
exec /app/buildbtw-backend "$@"
