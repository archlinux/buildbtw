#!/usr/bin/env bash
# Fetch our third-party web assets so we can vendor them.
# This way, we can serve it ourselves without having to depend on outside servers.
set -euo pipefail

DATASTAR_VERSION="v1.0.4"
BULMA_VERSION="1.0.4"
FONT_AWESOME_VERSION="7.3.1"
FONT_AWESOME_WEBFONTS=(fa-brands-400 fa-regular-400 fa-solid-900 fa-v4compatibility)

DEST_ROOT="assets/vendor"

wget -nv -O "$DEST_ROOT/datastar/datastar.js" "https://cdn.jsdelivr.net/gh/starfederation/datastar@$DATASTAR_VERSION/bundles/datastar.js"
wget -nv -O "$DEST_ROOT/bulma/bulma.min.css" "https://cdn.jsdelivr.net/npm/bulma@$BULMA_VERSION/css/bulma.min.css"
wget -nv -O "$DEST_ROOT/fontawesome/css/all.min.css" "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/$FONT_AWESOME_VERSION/css/all.min.css"
for font in "${FONT_AWESOME_WEBFONTS[@]}"; do
  wget -nv -O "$DEST_ROOT/fontawesome/webfonts/$font.woff2" "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/$FONT_AWESOME_VERSION/webfonts/$font.woff2"
done
