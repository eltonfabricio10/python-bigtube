#!/usr/bin/env bash
# Build a self-contained BigTube AppImage from an already-staged install tree.
#
# Usage (from the repo root, after `cargo build --release`):
#   bash rust/packaging/stage-tree.sh /tmp/stage
#   bash rust/packaging/build-appimage.sh /tmp/stage "$VERSION"
#
# Bundles GTK4/libadwaita and the GStreamer plugins (including the gtk4 paintable
# sink the player needs) via linuxdeploy + its gtk/gstreamer plugins, so the
# AppImage runs on any reasonably current x86_64 glibc system without installing
# GTK from the distro. yt-dlp/deno are still fetched at runtime into the user's
# data dir, so they are intentionally NOT bundled.
set -euo pipefail

stage="${1:?usage: build-appimage.sh <stage-dir> <version>}"
version="${2:?usage: build-appimage.sh <stage-dir> <version>}"
appid="io.github.eltonfabricio10.bigtube"

workdir="${APPIMAGE_WORKDIR:-$PWD/.appimage}"
tools="$workdir/tools"
appdir="$workdir/AppDir"
mkdir -p "$tools"
rm -rf "$appdir"
mkdir -p "$appdir"
cp -a "$stage/usr" "$appdir/usr"

# Fetch linuxdeploy and the GTK + GStreamer plugins (cached in $tools).
fetch() {
  local url="$1" out="$tools/$2"
  [ -f "$out" ] || { echo "downloading $2"; curl -fsSL -o "$out" "$url"; }
  chmod +x "$out"
}
# linuxdeploy ships as a release AppImage; the plugins are raw scripts in their
# repos (no release asset), so pull those from the default branch.
gh="https://github.com/linuxdeploy"
raw="https://raw.githubusercontent.com/linuxdeploy"
fetch "$gh/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" linuxdeploy
fetch "$raw/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh" linuxdeploy-plugin-gtk.sh
fetch "$raw/linuxdeploy-plugin-gstreamer/master/linuxdeploy-plugin-gstreamer.sh" linuxdeploy-plugin-gstreamer.sh

export PATH="$tools:$PATH"
# Nested AppImages (linuxdeploy, appimagetool) run without FUSE — works in CI.
export APPIMAGE_EXTRACT_AND_RUN=1
# Tell the GTK plugin we use GTK 4.
export DEPLOY_GTK_VERSION=4
# linuxdeploy bundles an old `strip` that chokes on modern `.relr.dyn` sections
# (glibc 2.36+ / recent binutils), aborting the build. Skip stripping — our own
# binary is already stripped by the release profile and the libs are system libs.
export NO_STRIP=1
export OUTPUT="BigTube-${version}-x86_64.AppImage"

"$tools/linuxdeploy" \
  --appdir "$appdir" \
  --desktop-file "$appdir/usr/share/applications/${appid}.desktop" \
  --icon-file "$appdir/usr/share/icons/hicolor/512x512/apps/bigtube.png" \
  --plugin gtk \
  --plugin gstreamer \
  --output appimage

echo "built: $OUTPUT"
ls -lh "$OUTPUT"
