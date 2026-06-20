#!/usr/bin/env bash
# Assemble the install tree (rooted at "$1/usr") shared by every package format:
# the Arch prebuilt tarball, the .deb and the .rpm. Run from the repo root, after
# `cargo build --release`. Keeps the layout identical across distros so a glitch
# in one format can't silently diverge from the others.
set -euo pipefail

stage="${1:?usage: stage-tree.sh <stage-dir>}"

install -Dm755 rust/target/release/bigtube-gui "$stage/usr/bin/bigtube-gui"
install -Dm755 rust/target/release/bigtube     "$stage/usr/bin/bigtube"

install -Dm644 rust/packaging/io.github.eltonfabricio10.bigtube.desktop \
  "$stage/usr/share/applications/io.github.eltonfabricio10.bigtube.desktop"

install -Dm644 src/bigtube/data/bigtube.png \
  "$stage/usr/share/icons/hicolor/512x512/apps/bigtube.png"
install -Dm644 src/bigtube/data/bigtube.svg \
  "$stage/usr/share/icons/hicolor/scalable/apps/bigtube.svg"
# Also install the icon under the app ID so the window icon (which KDE and others
# resolve by app_id) shows up, not just the launcher icon.
install -Dm644 src/bigtube/data/bigtube.png \
  "$stage/usr/share/icons/hicolor/512x512/apps/io.github.eltonfabricio10.bigtube.png"
install -Dm644 src/bigtube/data/bigtube.svg \
  "$stage/usr/share/icons/hicolor/scalable/apps/io.github.eltonfabricio10.bigtube.svg"

for po in po/*.po; do
  [ -e "$po" ] || continue
  lang="$(basename "$po" .po)"
  install -d "$stage/usr/share/locale/$lang/LC_MESSAGES"
  msgfmt "$po" -o "$stage/usr/share/locale/$lang/LC_MESSAGES/bigtube.mo"
done

install -Dm644 LICENSE "$stage/usr/share/licenses/bigtube/LICENSE"
