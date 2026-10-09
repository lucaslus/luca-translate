# Lucas Translate — native Omarchy plugin

Omarchy uses QML and Qt Quick inside the existing Omarchy Shell. A separate Rust
process supplies translation, dictionaries, credentials, history and local OCR.
The native build has no Tauri or WebView dependency.

The interface binds to the installed Shell's `qs.Commons.Color`, `Style` and
`Border` and uses `qs.Ui` controls. There is no application theme, Dark/Light
selector or stored native theme preference. Theme changes update the open panel
without replacing its input, results or running requests. Fonts, spacing and
surface styling follow the Shell's effective settings, including user overrides.
The compact panel is 440 logical pixels wide at the default Shell scale, with
a short empty state that expands for results. The title-free toolbar uses uniform vector icons for history, favorites and
settings, with named navigation tooltips; result sections use separators rather than nested
boxes, and duplicate dictionary paragraphs are displayed once while remaining
available for copy and favorites. Copy and favorite actions use icon buttons with
tooltips; saved favorites show a filled star. An overflow fade and downward icon
appear only while additional translations remain below the viewport; clicking the
icon advances the results and reaching the bottom hides it. Settings use compact
switches with provider toggles arranged in two columns. Secondary text retains
the theme's muted color when readable, otherwise
blending it toward the theme's popup text to reach 4.5:1 contrast when that text
color supports it. This also covers input placeholders and metadata. Language controls and the Clear/Translate icons share one row between
the input and results; Translate becomes a stop icon while a request is running. Capture actions use their existing desktop shortcuts without
adding buttons to the translation panel. AI and DeepL API forms appear only when
their switches are enabled.

## Runtime requirements

- Omarchy Quickshell plugin runtime with `Commons`, `Ui.BorderSurface`,
  `Ui.Dropdown`, `Ui.TextField`, `Ui.Toggle`, panel and service entry points.
  Verified locally on Omarchy `4.0.0.alpha` / Quickshell `0.3.1` on 2026-10-03.
- `python`, `wl-clipboard`, `grim`, `slurp`, `tesseract`,
  `tesseract-data-eng`, `tesseract-data-chi_sim`.
- An unlocked Secret Service for API keys; keys never enter IPC read responses.
- `mpv` for dictionary pronunciation; `omarchy` and `lucas-screenshot-editor`
  for the existing system screenshot/edit workflow.

## Install from this source checkout

Unlock the desktop before installing or updating the plugin. The locally
installed Shell can abort when plugin changes reload a locked session; the
installer checks the lock IPC before modifying files.
If an installed Shell retains cached QML after an update, run
`omarchy restart shell` while the desktop is unlocked to load the new components.

From the repository root:

```sh
python3 scripts/install-omarchy-native.py
```

The installer builds only `crates/lucas-omarchy`, backs up the previous desktop
configuration, installs `~/.config/omarchy/plugins/lucas.translate`, installs the
backend and launcher to `~/.local/bin`, adds the bar entry, and switches the six
existing shortcuts to the native launcher. It checks actual discovery and
backend readiness before switching shortcuts, validates the compositor reload,
and restores managed files if installation fails. Occupied shortcuts are left
unassigned and reported in native Settings.

For a backend built separately:

```sh
python3 scripts/install-omarchy-native.py \
  --binary crates/lucas-omarchy/target/release/lucas-translate-omarchy-backend
```

`--without-shortcuts` installs the plugin without changing Hyprland bindings.
The user desktop entry and `~/.local/bin/lucas-translate` dispatch to the native
version only in an Omarchy graphical session. Other Linux sessions dispatch to
the existing system desktop executable, including Hyprland sessions without a
running Omarchy Shell. The Tauri package and source remain
available for other platforms; native code does not alter their theme settings.

Build a standalone native bundle with `bash scripts/build-omarchy-native.sh`.
Extract its archive and run
`python3 scripts/install-omarchy-native.py --binary bin/lucas-translate-omarchy-backend`
from the extracted `lucas-translate-omarchy` directory. The bundle contains the
plugin, installer and backend; installation does not require the Tauri package
or a Rust compiler.

## Usage

```sh
lucas-translate-native --show
lucas-translate-native --toggle
lucas-translate-native --input
lucas-translate-native --selection
lucas-translate-native --screenshot
lucas-translate-native --ocr
lucas-translate-native --annotate
```

`Enter` translates the input; `Shift+Enter` adds a newline. Escape cancels an
active translation and hides the panel. Input focus is restored on opening,
returning to Translate, selecting a language, submitting or clearing input.
An Enter used to confirm an active input-method composition remains available
to the input method; press Enter again after the text is committed. Holding Enter
does not repeatedly restart translations. The command
`omarchy-shell shell call lucas.translate health input` reports only focus,
composition and text length for diagnosing input, without exposing the query.
Outside-click or Close hides it while
retaining its input and allowing translation to finish. OCR-to-clipboard
does not open the panel or invoke translation. Cancelling a region capture
leaves the clipboard unchanged. Screenshot annotation reuses the previous system flow: Omarchy's region picker
and the installed `lucas-screenshot-editor`. Enter confirms to the clipboard and
closes; Escape, Cancel or outside-click discards the image. The editor retains its
existing save controls. Capture files are private and removed when the editor
closes, including cancellation and failures. Editing has no forced time limit.
Backend shutdown kills the editor and removes temporary files.
Selection capture requires the source application
to expose its live selection via accessibility or respond to its Copy shortcut.
The selection action pins the focused window at invocation, clears old results,
and opens an empty input panel when no selection is available. It never uses a
persisted PRIMARY offer or existing clipboard text as a query. Accessibility
reads leave the clipboard untouched; the Copy fallback accepts only a new offer
and restores all original clipboard MIME formats before translating. Terminals
use Ctrl+Shift+C. Editors known to copy a whole line without a selection require
accessibility; otherwise the action opens empty input.
Copy fallback recognizes native app IDs and executable names, including Feishu
windows with an empty app ID. Synthetic Copy uses Omarchy's explicit key-down
and delayed key-up sequence.

Settings includes service toggles, language routing, local/OpenAI-compatible AI,
DeepL API with a saved-key connection test, interface language, editable ordered
language rules, desktop shortcuts and a local diagnostics entry. Appearance is always managed
by Omarchy. History and favorites support paging, copy and retranslating; history
deletion requires a second confirmation. Dictionary text and results are rendered
as plain text, including markup-like provider content.

Language rules can be added, removed and reordered, and are saved explicitly;
unrelated settings refreshes preserve unfinished rule edits. Native shortcuts
use the existing managed Hyprland combinations, including saved custom values;
the settings editor currently accepts combination text instead of key recording.
Plugin enablement and startup are controlled by Omarchy Shell, replacing the
desktop application's login-autostart setting.

## Data and isolation

Native data lives in `$XDG_DATA_HOME/lucas-translate-omarchy` (default
`~/.local/share/lucas-translate-omarchy`). First startup copies existing settings
and takes a SQLite snapshot of `com.lucas.translate/lucas.db`, including committed
WAL data. Original data remains intact. Native interface preferences exclude
the old app's theme, font-size and shortcut settings. Native shortcuts are
managed through Hyprland independently. Logs contain allowlisted metadata,
not input, translations or credentials.

Native keys use a separate Secret Service namespace. Imported desktop key ids
are copied into that namespace on first use; replacing or clearing a native key
does not delete the original desktop credential.

The backend reuses the unchanged `lucas-core`. Its host modules were adapted from
the desktop implementation into a separate crate, so the native runtime can be
rebuilt without changing the Tauri host. Improvements common to both variants
should subsequently be extracted deliberately rather than changing the desktop
host as a side effect of native work.

macOS, Windows, ordinary Linux and the existing desktop build/release workflow
continue to use the existing Tauri UI and Dark/Light logic. Native CI produces
an independent backend and plugin archive; it does not replace desktop artifacts.

## Verification

```sh
cargo test --manifest-path crates/lucas-omarchy/Cargo.toml --locked
cargo build --manifest-path crates/lucas-omarchy/Cargo.toml --locked
python3 omarchy/tests/backend_test.py
python3 -B omarchy/tests/stability_test.py
python3 omarchy/tests/shortcuts_test.py
python3 -B omarchy/tests/integration_test.py
python3 scripts/test-omarchy-native.py
python3 -B scripts/test-omarchy-ui.py
python3 -B scripts/test-omarchy-bridge.py
omarchy plugin validate omarchy
```

Backend tests use a local HTTP fixture and fake clipboard/capture tools; they
do not call public providers or modify the real clipboard. The QML test loads
actual Shell components in a temporary, windowless test configuration and
checks theme changes, delayed service injection, stale-result rejection and
preservation of input/results/running requests. CI runs backend and shortcut
checks; QML verification additionally requires the installed Omarchy modules
and Qt Test. The interaction runner sends mouse and keyboard events to the
actual controls in an offscreen Mesa window. The bridge runner additionally
connects the actual QML service to the Rust binary and a localhost provider.
Capture, OCR and clipboard executables are replaced with isolated fixtures.
JSON reports and logs are saved under `dist/omarchy/test-results`.

Stability checks exercise malformed transport/provider responses, rate limits,
overload, cancellation, process shutdown, concurrent configuration writes,
system editor confirm/cancel/failure/cleanup, bounded memory/file-descriptor use, repeated
theme changes and panel lifecycles. Missing noninteractive command replies time out, pending
UI requests are bounded, and pending capture state is released after backend exit. Installer tests intercept all desktop commands and verify restoration
after validation, discovery, enablement, shortcut and final-state failures.

These tests do not establish interactive Wayland focus, real region selection,
physical keybindings, live provider/keyring access, hardware rendering or
long-running stability. Those require an unlocked desktop smoke test.

## Restore the previous desktop integration

Installation prints its backup directory under
`~/.local/state/lucas-translate-omarchy/install-backups/`. It contains the previous
plugin (if present), launcher/backend, desktop entry, `shell.json`, Hyprland Lua
and managed shortcut state. Disable `lucas.translate` and restore the appropriate
snapshot when reverting. Keep native data if you intend to reinstall.
