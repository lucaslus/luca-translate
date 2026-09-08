# Release and update checklist

## Signing model

- macOS bundles use ad-hoc signing (`-`), not Developer ID, notarization, or App Store distribution. Windows installers also lack a publisher certificate. Platform trust prompts can occur.
- Tauri's separate updater key verifies downloaded packages before installation. Never disable this verification. Public key: `plugins.updater.pubkey` in `app/src-tauri/tauri.conf.json`.
- Private key is **not in the repository**. The maintainer's local restricted backup directory is `~/.local/share/luca-translate-updater/` (directory mode 700, key mode 600). GitHub Actions secret: `TAURI_SIGNING_PRIVATE_KEY`. The generated key has no password; its security depends on filesystem/account and GitHub secret access. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is optional for a future password-protected key.
- Back up the private key securely offline. Do not casually regenerate it: existing installs trust the embedded public key and cannot accept an unrelated replacement key. Do not print keys in CI logs.

## Build without publishing

CI `check.yml` tests macOS ARM64/Intel, Windows x64 and Ubuntu x64, then builds **release-mode installers without updater artifacts** (no secrets available to PR builds). These are test artifacts, not public releases or runtime certification.

Local unsigned-identity test build, from `app/`:

```sh
npx tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}' -- --locked
```

For project-signed updater archives, set `TAURI_SIGNING_PRIVATE_KEY` to the private key **file path** and run the same build without overriding `createUpdaterArtifacts`. Do not pass private key contents on command lines.

Offline verification (also asserts a tampered in-memory copy fails): `cargo run --manifest-path app/src-tauri/Cargo.toml --example verify_update -- path/to/archive`. This reads the public key embedded in the app configuration; it never installs an update.

## Prepare a release

1. Update the same stable version in `app/src-tauri/tauri.conf.json` and `app/src-tauri/Cargo.toml`; regenerate the app Cargo lockfile. Never reuse an already published version.
2. Merge reviewed changes to `main`. Run **Build signed release draft** manually from Actions on `main`.
3. All four build jobs must succeed. The aggregation step requires one signed updater archive per platform, unique asset names, and matching version metadata. It creates `latest.json` and `SHA256SUMS` only when all required assets exist.
4. A **draft only** is created. An existing tag/release causes creation to fail rather than overwriting published assets. A rerun requires reviewing/removing only the intended unfinished draft, or selecting a fresh version.
5. Download and manually smoke-test each target below. Publish the draft yourself only after verifying the assets. No workflow automatically publishes it.

The updater endpoint is `https://github.com/lucaslus/luca-translate/releases/latest/download/latest.json`. A draft or missing release is not an update feed. The first public version bootstraps distribution; a second higher version is required to test a real upgrade.

| Target | Installer | Updater archive |
| --- | --- | --- |
| macOS Apple Silicon | DMG | signed `.app.tar.gz` |
| macOS Intel | DMG | signed `.app.tar.gz` |
| Windows x64 | NSIS `.exe` | signed `.exe` |
| Linux x64 | AppImage, DEB, RPM | signed AppImage only |

Linux builds use Ubuntu 22.04 as a baseline. AppImage users need FUSE or `APPIMAGE_EXTRACT_AND_RUN=1`; OCR/selection still need `xclip`, Tesseract, `eng` / `chi_sim` language data, and an unlocked Secret Service for API credentials. Wayland is not fully supported; use X11. Windows OCR languages must be installed in Windows.

## Manual smoke tests before publishing

- Fresh install downloaded through a browser (quarantine/SmartScreen): platform trust prompt is understandable; only permit a trusted release. Never recommend disabling Gatekeeper globally.
- Launch twice: one process, existing panel shown; occupied default shortcut does not crash startup; settings exposes the conflict and replacement takes effect.
- Resize, move, close to tray, relaunch; ensure window returns on-screen. Escape preserves text and closes layers/cancels work before hiding the panel.
- macOS: first TCC grant and consecutive screenshot selections on **both ARM64 and Intel**. Cancel selection; empty OCR; mixed-language text; permission revoked; hotkey during OCR.
- Translate, cancel a stalled provider, repeat a successful query, change target/API account. Only matching successful results hit the bounded 5-minute memory cache.
- Configure DeepL API with a test account; incorrect Free/Pro plan, missing/invalid key, exhausted quota, locked credential store; free channels remain independent. Connection test uses `/v2/usage`, not billable translation text.
- Update: no published feed, offline, same version, unsupported architecture, download cancel, corrupt signature, unwritable disk, then install a real higher signed version. Tampered packages must never install. Confirm preferences, vault keys and history survive. Linux DEB/RPM must direct to manual updates.
- Compact main window, larger text, both themes, settings at minimum size, visible focus, no wrapped button labels or raw error URLs.

References: [Tauri Updater](https://v2.tauri.app/plugin/updater/), [Tauri AppImage](https://v2.tauri.app/distribute/appimage/), [Apple Gatekeeper](https://support.apple.com/en-us/102445).
