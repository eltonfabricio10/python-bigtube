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
# appimagetool packages the AppDir in a separate step (we package by hand so the
# RELR fixup below lands between deploy and packaging).
fetch "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" appimagetool

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

# Deploy GTK/GStreamer into the AppDir, but DON'T package yet — we package after
# the RELR fixup below.
"$tools/linuxdeploy" \
  --appdir "$appdir" \
  --desktop-file "$appdir/usr/share/applications/${appid}.desktop" \
  --icon-file "$appdir/usr/share/icons/hicolor/512x512/apps/bigtube.png" \
  --plugin gtk \
  --plugin gstreamer

# ---------------------------------------------------------------------------
# Undo patchelf's RELR corruption.
#
# linuxdeploy runs `patchelf --set-rpath` on every bundled lib/helper to make
# them find each other. patchelf 0.18 mangles ELF objects that use RELR
# (relative-relocation) sections — the default on glibc>=2.36 distros (Arch,
# Ubuntu 23.10+, Fedora 38+). The result segfaults inside ld-linux during
# relocation, *before* main(), so the AppImage won't even start on those hosts.
#
# Fix: restore every RELR object to its pristine system copy (byte-identical to
# what linuxdeploy bundled, minus the broken rpath) and let an AppRun hook set
# LD_LIBRARY_PATH so they still find their bundled deps on any host.
# ---------------------------------------------------------------------------
echo "restoring RELR objects mangled by patchelf..."
declare -A SYSLIB
while IFS= read -r p; do
  b=$(basename "$p"); [ -n "${SYSLIB[$b]:-}" ] || SYSLIB[$b]="$p"
done < <(find /usr/lib /usr/lib64 /lib /lib64 -xtype f \
           \( -name '*.so*' -o -name 'gst-plugin-scanner' -o -name 'gst-ptp-helper' \) 2>/dev/null)
restored=0
while IFS= read -r obj; do
  readelf -d "$obj" 2>/dev/null | grep -q '(RELR)' || continue
  src="${SYSLIB[$(basename "$obj")]:-}"
  [ -n "$src" ] || continue            # no system twin (e.g. our own binary) -> leave it
  cp -f --no-preserve=mode "$(readlink -f "$src")" "$obj" && restored=$((restored+1))
done < <(find "$appdir/usr/lib" -type f \( -name '*.so*' -o -name 'gst-plugin-scanner' -o -name 'gst-ptp-helper' \))
echo "  restored $restored objects"

# The LADSPA plugin crashes on load in a bundled context (host LADSPA deps) and
# BigTube never uses audio-effect plugins, so drop it — keeps the registry scan
# clean instead of relying on the scanner to blacklist it.
find "$appdir/usr/lib" -name 'libgstladspa.so' -delete 2>/dev/null || true

# Bundle the Adwaita icon theme. linuxdeploy doesn't bundle icon themes, so a
# libadwaita app loses its symbolic icons (window controls, etc.) on hosts that
# don't ship Adwaita. Copy it (and a hicolor index) into the AppDir.
mkdir -p "$appdir/usr/share/icons"
for theme in Adwaita hicolor; do
  src="/usr/share/icons/$theme"
  [ -d "$src" ] && cp -an "$src" "$appdir/usr/share/icons/" 2>/dev/null || true
done
command -v gtk-update-icon-cache >/dev/null 2>&1 \
  && gtk-update-icon-cache -qtf "$appdir/usr/share/icons/Adwaita" 2>/dev/null || true

# Runtime env so the pristine (rpath-less) libs resolve, and so GStreamer finds
# its out-of-process scanner (which sandboxes any crashy plugin instead of
# taking the app down). linuxdeploy's own gstreamer hook points the scanner at
# the wrong subdir, so we compute the real paths from the AppDir here.
scanner_rel=$(cd "$appdir" && find usr/lib -type f -name gst-plugin-scanner | head -1)
plugin_rel=$(cd "$appdir" && dirname "$(find usr/lib -type f -name 'libgstcoreelements.so' | head -1)")
cat > "$appdir/apprun-hooks/zz-bigtube-fixup.sh" <<EOF
export LD_LIBRARY_PATH="\$APPDIR/usr/lib:\$APPDIR/usr/lib/x86_64-linux-gnu\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
export GST_PLUGIN_SCANNER="\$APPDIR/${scanner_rel}"
export GST_PLUGIN_SCANNER_1_0="\$APPDIR/${scanner_rel}"
export GST_PLUGIN_SYSTEM_PATH_1_0="\$APPDIR/${plugin_rel}"
export GST_PLUGIN_PATH_1_0="\$APPDIR/${plugin_rel}"
# BigTube is a libadwaita app. linuxdeploy's gtk hook forces GTK_THEME=Adwaita:<variant>,
# which layers the legacy GTK Adwaita CSS on top of libadwaita's own stylesheet and
# renders broken/ugly. Unset it so libadwaita styles natively (it still picks up
# light/dark from the desktop portal, which the gtk hook already queried).
unset GTK_THEME
EOF
# Make sure the hook is sourced by AppRun.
if ! grep -q zz-bigtube-fixup "$appdir/AppRun"; then
  sed -i 's#\(source "\$this_dir"/apprun-hooks/"linuxdeploy-plugin-gtk.sh"\)#\1\nsource "$this_dir"/apprun-hooks/"zz-bigtube-fixup.sh"#' "$appdir/AppRun"
fi

# Package the fixed AppDir.
ARCH=x86_64 "$tools/appimagetool" "$appdir" "$OUTPUT"

echo "built: $OUTPUT"
ls -lh "$OUTPUT"
