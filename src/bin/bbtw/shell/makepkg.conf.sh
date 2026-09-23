#!/usr/bin/env bash

MAKEPKG_CONF=${MAKEPKG_CONF:-/etc/makepkg.conf}

if [[ -r $MAKEPKG_CONF ]]; then
    # shellcheck disable=SC1090
    source "$MAKEPKG_CONF"
    if [[ -d $MAKEPKG_CONF.d ]]; then
        for config in "$MAKEPKG_CONF.d"/*.conf; do
            # shellcheck disable=SC1090
            source "${config}"
        done
    fi
fi

XDG_PACMAN_HOME="${XDG_CONFIG_HOME:-$HOME/.config}/pacman"
if [[ $MAKEPKG_CONF == /etc/makepkg.conf ]]; then
    if [[ -r $XDG_PACMAN_HOME/makepkg.conf ]]; then
        # shellcheck disable=SC1091
        source "$XDG_PACMAN_HOME/makepkg.conf"
    elif [[ -r $HOME/.makepkg.conf ]]; then
        # shellcheck disable=SC1091
        source "$HOME/.makepkg.conf"
    fi
fi

if [[ -n $PKGDEST ]]; then
    printf "%s" "$(realpath "${PKGDEST}")"
fi

