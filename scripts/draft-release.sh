#!/usr/bin/env bash
set -euo pipefail

: "${GH_REPO:?Set GH_REPO to the release repository}"
: "${RELEASE_TAG:?Set RELEASE_TAG to an existing version tag}"
[[ "$RELEASE_TAG" == v* ]] || { echo 'Expected a v-prefixed tag' >&2; exit 1; }
asset_dir="${1:?Provide the prepared release assets directory}"

# Verify the files before any remote writes.
(cd "$asset_dir" && sha256sum --check SHA256SUMS.txt)

asset_url() {
  local name=${1:?Provide an asset name}
  printf 'https://github.com/%s/releases/download/%s/%s' "$GH_REPO" "$RELEASE_TAG" "$name"
}

find_asset() {
  local pattern=${1:?Provide an asset pattern}
  local match
  match=$(find "$asset_dir" -maxdepth 1 -type f -name "$pattern" -print -quit)
  [[ -n "$match" ]] || { echo "Missing release asset matching $pattern" >&2; exit 1; }
  basename "$match"
}

mac_x64=$(find_asset '*_x64.dmg')
mac_arm64=$(find_asset '*_aarch64.dmg')
windows_x64=$(find_asset '*_x64-setup.exe')
windows_arm64=$(find_asset '*_arm64-setup.exe')
linux_x64=$(find_asset '*_amd64.AppImage')
linux_arm64=$(find_asset '*_aarch64.AppImage')

notes_file=$(mktemp)
trap 'rm -f "$notes_file"' EXIT
cat >"$notes_file" <<EOF
## Download PrepLoop

Choose the one installer that matches your device:

- **macOS · Intel 64-bit:** [Download the Intel DMG]($(asset_url "$mac_x64"))
- **macOS · Apple Silicon:** [Download the ARM64 DMG]($(asset_url "$mac_arm64"))
- **Windows · Intel/AMD 64-bit:** [Download the Windows installer]($(asset_url "$windows_x64"))
- **Windows · ARM64:** [Download the Windows ARM installer]($(asset_url "$windows_arm64"))
- **Linux · Intel/AMD 64-bit:** [Download the AppImage]($(asset_url "$linux_x64"))
- **Linux · ARM64:** [Download the AppImage]($(asset_url "$linux_arm64"))

Not sure which one to choose? Most Windows and Linux users need Intel/AMD 64-bit. Macs from late 2020 onward usually use Apple Silicon.

### Technical files

\`latest.json\` and the macOS \`.app.tar.gz\` archives support in-app updates. \`SHA256SUMS.txt\` supports integrity checks. You do not need to download these files for a normal installation.

**Full Changelog**: [https://github.com/$GH_REPO/commits/$RELEASE_TAG](https://github.com/$GH_REPO/commits/$RELEASE_TAG)
EOF

if draft=$(gh release view "$RELEASE_TAG" --json isDraft --jq '.isDraft'); then
  if [[ "$draft" != true ]]; then
    echo "Refusing to modify published release $RELEASE_TAG" >&2
    exit 1
  fi
else
  # --verify-tag prevents accidentally creating a tag at a different commit.
  # If the lookup failed for any reason other than absence, creation fails safely.
  gh release create "$RELEASE_TAG" --verify-tag --draft \
    --title "PrepLoop $RELEASE_TAG" --notes-file "$notes_file" --generate-notes
fi

# A rerun must remove assets from older matrix versions; otherwise the public
# release page slowly accumulates obsolete or confusing downloads.
while IFS= read -r remote_asset; do
  [[ -n "$remote_asset" ]] || continue
  [[ -f "$asset_dir/$remote_asset" ]] && continue
  echo "Removing obsolete release asset: $remote_asset"
  gh release delete-asset "$RELEASE_TAG" "$remote_asset" --yes
done < <(gh release view "$RELEASE_TAG" --json assets --jq '.assets[].name')

# Preserve manually edited notes on reruns. Never publish from CI.
gh release upload "$RELEASE_TAG" "$asset_dir"/* --clobber
gh release view "$RELEASE_TAG" --json url --jq '.url'
