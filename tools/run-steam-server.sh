#!/usr/bin/env bash
set -euo pipefail
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$root"
export VOXEL_STEAM_APP_ID=${VOXEL_STEAM_APP_ID:-480}
export SteamAppId=$VOXEL_STEAM_APP_ID
export VOXEL_STEAM_DEDICATED=1
export HOME=${VOXEL_STEAM_RUNTIME_HOME:-"$root/runtime-home"}
export VOXEL_SAVE_DIR=${VOXEL_SAVE_DIR:-"$root/saves"}
export LD_LIBRARY_PATH="$root/lib:$root/linux64:${LD_LIBRARY_PATH:-}"
mkdir -p "$HOME/.steam/sdk64" "$VOXEL_SAVE_DIR"
if [ ! -e "$HOME/.steam/sdk64/steamclient.so" ]; then
  ln -s "$root/linux64/steamclient.so" "$HOME/.steam/sdk64/steamclient.so"
fi
exec "$root/voxel-server" --multiplayer-headless --multiplayer-host=steam "$@"
