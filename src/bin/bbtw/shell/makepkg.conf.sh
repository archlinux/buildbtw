#!/usr/bin/env bash

#  Copyright (c) 2006-2025 Pacman Development Team <pacman-dev@lists.archlinux.org>
#  Copyright (c) 2002-2006 by Judd Vinet <jvinet@zeroflux.org>
#
#  This program is free software; you can redistribute it and/or modify
#  it under the terms of the GNU General Public License as published by
#  the Free Software Foundation; either version 2 of the License, or
#  (at your option) any later version.
#
#  This program is distributed in the hope that it will be useful,
#  but WITHOUT ANY WARRANTY; without even the implied warranty of
#  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#  GNU General Public License for more details.
#
#  You should have received a copy of the GNU General Public License
#  along with this program.  If not, see <http://www.gnu.org/licenses/>.
#
#  ---
#  The original work has been modified at 2026-09-30
#  ---
#
#  This script helps to bridge the makepkg config parsing by loading
#  what essentially are shell scripts with potential expansion and
#  print config values like PKGDEST so the Rust side can consume it
#  while no native parser exists.

shopt -s nullglob

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
    # Try to normalize with best effort, ignoring missing directories
    printf "%s" "$(realpath --canonicalize-missing "${PKGDEST}")"
fi

