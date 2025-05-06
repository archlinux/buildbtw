#!/usr/bin/bash
set -o nounset -o errexit -o pipefail -o xtrace

REPO_URL=$1

pacman --noconfirm -Sy devtools

# Add buildbtw repo for this namespace
sed -i "$ a [buildbtw-namespace]\nServer = $REPO_URL" /usr/share/devtools/pacman.conf.d/*

# Create user to run the build as non-root
# but give them sudo access because it actually does need root
useradd -m -p '' builder
echo 'builder ALL=(ALL:ALL) NOPASSWD: ALL' >> /etc/sudoers

cp -R /mnt/src_repo /build
cd /build
chown -R builder .

# Run build
sudo -u builder bash << EOF
pkgctl build .
EOF
