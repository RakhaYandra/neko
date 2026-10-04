# Neko

> Your little OpenCode companion for Linux.

A local-first desktop companion that reflects [OpenCode](https://opencode.ai)
session state — working, waiting for permission, completed, error — so you do
not have to switch back to the terminal to find out what is happening.

## What it does

* **Shows live session state** for every OpenCode session on your desktop.
* **Lets you answer permission requests** from the desktop: Allow once, Always,
  or Deny.
* **Notifies only what matters**: permission requested, session completed,
  session error.
* **Keeps running when OpenCode does.** If Neko is closed, your agent is
  unaffected.

Everything is local: a Unix socket with `0600` permissions, no TCP port, no
telemetry, no cloud, no source code or prompts stored on disk.

## Requirements

* Linux (developed and tested on Hyprland/Wayland; X11 untested)
* [OpenCode](https://opencode.ai) 1.18+ (plugin API `event`, permission events)
* `opencode serve` running **if** you want to answer permissions (see below)

## Install

Download the release asset and run it:

```sh
# AppImage
chmod +x Neko_0.1.0_amd64.AppImage && ./Neko_0.1.0_amd64.AppImage

# or Debian package
sudo apt install ./Neko_0.1.0_amd64.deb
```

Or build from source:

```sh
npm install
npm run build --workspace @neko/opencode-plugin
npm run tauri --workspace @neko/desktop -- build
```

Bundles land in `apps/desktop/src-tauri/target/release/bundle/`.

## Set up the OpenCode plugin

Neko needs a plugin inside OpenCode to receive events. Build it once, then copy
the bundle into the OpenCode plugin directory:

```sh
npm run build --workspace @neko/opencode-plugin
cp packages/opencode-plugin/dist/index.js ~/.config/opencode/plugins/neko.js
```

`~/.config/opencode/plugins/` applies to every project; use
`<project>/.opencode/plugins/` instead to scope it to one repository.

## Answer permissions (optional)

Permission replies travel from Neko to OpenCode over the local serve API. Start
OpenCode as a server and point Neko at it:

```sh
opencode serve --port 18789 &
export NEKO_OPENCODE_URL=http://127.0.0.1:18789
neko   # or the AppImage
```

Without `NEKO_OPENCODE_URL`, Neko still shows everything, but permission
requests display "answer in the terminal" instead of buttons. TUI-only sessions
(`opencode`, without `serve`) are always observe-only.

### Safety

* A permission is answered **only** when you click a button. There is no
  auto-approve, no auto-reject, and no timer that decides for you.
* Neko targets the localhost URL you give it. It never scans ports and never
  contacts a remote host.
* If OpenCode is unreachable, the request stays pending — nothing is approved.
* Answers are sent to the terminal session too, so a decision made there is
  never overwritten.

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `NEKO_SOCK` | `$XDG_RUNTIME_DIR/neko.sock` (else `/tmp/neko.sock`) | Unix socket the plugin pushes events to. Explicit `NEKO_SOCK` wins; otherwise both sides use the same default, so restart the app and the plugin together after upgrading or events will go missing. |
| `NEKO_DB` | `$XDG_DATA_HOME/neko/neko.db` | SQLite location |
| `NEKO_OPENCODE_URL` | *(unset)* | OpenCode serve base URL for permission replies |

## Desktop integration

* System tray: session count, show/hide, notification and autostart toggles,
  settings, quit.
* Notifications: permission requested, completed, error (toggleable).
* Start on login: toggleable from the tray.
* Global shortcut: `Super+Alt+N` shows/hides the companion. If the shortcut is
  already taken, Neko logs it and runs without it.
* Settings: notifications, always-on-top, autostart, animations, opacity,
  recenter top-center.

## Development

```sh
npm install
npm run dev          # Tauri dev window
npm run test         # vitest + node test + cargo test
npm run lint
```

Headless event listener, useful for debugging the bridge:

```sh
cargo run --manifest-path apps/desktop/src-tauri/Cargo.toml --example neko-listen
# override only when debugging: NEKO_SOCK=/tmp/dbg.sock ... neko-listen
```

## Architecture in one diagram

```text
OpenCode  ──plugin (event hooks)──▶  NDJSON over Unix socket (0600)
                                             │
                                             ▼
                                   Rust core: validate → session
                                   state machine → SQLite → snapshot
                                             │
                                             ▼
                                    Tauri event  ──▶  React UI
```

The plugin is independent of the UI, and the UI never talks to OpenCode
directly. State transitions are owned by Rust; the UI only mirrors snapshots.

## Design

The mascot is original pixel art built for Neko. It is not derived from any
other desktop companion.

## License

See repository metadata. Documentation and internal design notes are not
published; the public repository contains the code only.