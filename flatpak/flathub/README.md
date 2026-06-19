# Flathub submission

This folder contains everything needed to publish BigTube on
[Flathub](https://flathub.org):

| File | Purpose |
|------|---------|
| `org.big.bigtube.yaml` | Offline, reproducible manifest (vendored Rust crates) |
| `cargo-sources.json`   | Vendored crates generated from `rust/Cargo.lock` |

The app's AppStream metadata and desktop entry live in the main repo at
`src/bigtube/data/org.big.bigtube.metainfo.xml` and
`src/bigtube/data/org.big.bigtube.desktop`.

---

## ⚠️ Before submitting: the App ID

Flathub requires the application ID to be based on a **domain or code-hosting
account you control**. `org.big.bigtube` maps to the domain `big.org`, which is
**not** ours — Flathub reviewers will reject it.

For a GitHub-hosted project without its own domain, the correct ID is:

```
io.github.eltonfabricio10.bigtube
```

Renaming touches the desktop file, the metainfo `<id>`/`<launchable>`, the
`StartupWMClass`, and the manifest filename (it must match the ID). Decide and
apply this rename **before** opening the Flathub PR. (Local/AUR builds can keep
`org.big.bigtube`, but the Flathub artifact must use the controlled ID.)

---

## Test the manifest locally

```bash
# One-time: runtimes + Rust SDK extension
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08

# Build offline exactly like Flathub's buildbot (no network for cargo)
flatpak-builder --user --install --force-clean --sandbox \
    build-dir flatpak/flathub/org.big.bigtube.yaml
flatpak run org.big.bigtube
```

The `--sandbox` flag forbids build-time network, proving the vendored sources
are complete (this is what Flathub's CI enforces).

---

## Submit to Flathub

1. Fork [`flathub/flathub`](https://github.com/flathub/flathub) and create a
   branch named exactly after the app ID, e.g. `new-pr` → branch
   `io.github.eltonfabricio10.bigtube`.
2. Add the manifest (`io.github.eltonfabricio10.bigtube.yaml`) and
   `cargo-sources.json` to the branch root.
3. Open a pull request against `flathub/flathub`. The Flathub bot builds the
   app and runs `flatpak run` smoke tests.
4. Address reviewer feedback. Once merged, Flathub creates a dedicated
   `flathub/io.github.eltonfabricio10.bigtube` repository that you maintain.

After it's live:

```bash
flatpak install flathub io.github.eltonfabricio10.bigtube
```

---

## Updating for a new release

1. If dependencies changed, regenerate the vendored crates:
   ```bash
   pip install aiohttp toml tomlkit
   curl -fsSLO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
   python flatpak-cargo-generator.py rust/Cargo.lock -o flatpak/flathub/cargo-sources.json
   ```
2. Bump the `tag:` and `commit:` of the `bigtube` git source in the manifest to
   the new release.
3. Update the `<releases>` block in the metainfo.
4. Open a PR on the `flathub/<app-id>` repository.
