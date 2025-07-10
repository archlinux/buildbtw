#!/usr/bin/bash
set -o nounset -o pipefail -o xtrace -o errexit

pacman --noconfirm -Syu devtools

# Add custom buildbtw repo for this namespace.
# This can be disabled by passing "no_custom_repo".
REPO_URL=$1
if [[ "$REPO_URL" != "no_custom_repo" ]]; then
    sed -i "$ a [buildbtw-namespace]\nServer = $REPO_URL" /usr/share/devtools/pacman.conf.d/*
fi

# Create user to run the build as non-root
# but give them sudo access because it actually does need root
useradd -m -p '' builder
echo 'builder ALL=(ALL:ALL) NOPASSWD: ALL' >> /etc/sudoers

# Setup up working directory for build with correct permissions
cp -R /mnt/src_repo /build
cd /build
chown -R builder .

# Import upstream GPG keys
(
    set +eu
    . PKGBUILD
    if (( ${#validpgpkeys[@]} )); then
        keyservers=(
            hkps://keys.openpgp.org
            hkps://keyserver.ubuntu.com
        )
        for keyserver in "${keyservers[@]}"; do
            sudo -u builder gpg --keyserver "$keyserver" --recv-keys "${validpgpkeys[@]}"
        done
    fi
)

# Run build
export PKGDEST="/mnt/output/"
sudo --preserve-env="PKGDEST" -u builder pkgctl build
