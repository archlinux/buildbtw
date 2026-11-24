#!/usr/bin/bash
set -o nounset -o pipefail -o errexit

BUILDBTW_ENVIRONMENT=${1:-}
ARCHITECTURE=${2:-}
REPO_NAME=${3:-}
REPO_URL=${4:-}

echo "Installing devtools..."
pacman --noconfirm -Syu devtools

# Add buildbtw repo for this namespace
if [[ -n ${REPO_NAME} && -n ${REPO_URL} ]]; then
    echo "Adding buildbtw pacman repository ${REPO_NAME} at ${REPO_URL}..."
    sed -i "$ a [${REPO_NAME}]\nServer = $REPO_URL" /usr/share/devtools/pacman.conf.d/*
fi

# Create user to run the build as non-root
# but give them sudo access because it actually does need root
useradd -m -p '' builder
echo 'builder ALL=(ALL:ALL) NOPASSWD: ALL' >> /etc/sudoers

# Setup up working directory for build with correct permissions
cp -R -v /mnt/src_repo /build
cd /build
chown -R builder .

# Import upstream GPG keys
if [[ -d keys/pgp ]]; then
    sudo -u builder gpg --import keys/pgp/*.asc
fi

# export PACKAGER to have known identities, otherwise "Unknown Packager"
# is the default, which is often rejected in other tooling.
hostname=$(uname -n)
export PACKAGER="${hostname} <${hostname}@buildbtw>"

# define PKGDEST where the built output artifacts are expected
# to reach the caller.
export PKGDEST="/mnt/output/"

# Taint development/staging build artifacts
# https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/366
echo "Configured buildbtw environment: ${BUILDBTW_ENVIRONMENT}"
if [[ $BUILDBTW_ENVIRONMENT != production ]]; then
    printf '\n# %s\n' "$(openssl rand -hex 24)" >> PKGBUILD
fi

# Run build and preserve environment variables that need to be set
# in the guest process.
sudo --preserve-env="PKGDEST,PACKAGER" -u builder -- \
    pkgctl build --arch "${ARCHITECTURE}"

# Undo PKGBUILD tainting as the build artifacts are already tained after
# building and we may want to store the final PKGBUILD without exposing
# the taint secret.
if [[ $BUILDBTW_ENVIRONMENT != production ]]; then
    # 48 hex chars + 2 comment + 2 newlines
    truncate -s -52 -- PKGBUILD
fi
