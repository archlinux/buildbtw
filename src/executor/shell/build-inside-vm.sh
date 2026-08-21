#!/usr/bin/bash
set -o nounset -o pipefail -o xtrace -o errexit

ARCHITECTURE=${1:-}
REPO_NAME=${2:-}
REPO_URL=${3:-}

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

# Run build and preserve environment variables that need to be set
# in the guest process.
sudo --preserve-env="PKGDEST,PACKAGER" -u builder -- \
    pkgctl build --arch "${ARCHITECTURE}"
