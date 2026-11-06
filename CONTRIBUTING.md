# Contributing to Meron

## Prerequisites

- [Go](https://go.dev) 1.26+
- [Rust](https://www.rust-lang.org) (with `cargo`)
- [Bun](https://bun.sh)
- The [Wails v3 CLI](https://wails.io): `go install -tags gtk3 github.com/wailsapp/wails/v3/cmd/wails3@v3.1.0` (the `gtk3` tag preserves Meron's Linux native integrations).
- SQLite, and on Linux: GTK 3 + WebKitGTK 4.1

Desktop builds use `desktop/Taskfile.yml` and `scripts/build-desktop.ts`.
`desktop/wails.json` is retained only as shared release/version metadata; it is
not a Wails v2 build configuration. Linux Go commands must include `-tags gtk3`
to use the same GTK/WebKit backend as Meron's native integrations.

A [Nix shell](shell.nix) is provided with these dependencies:

```sh
nix-shell
```

## Develop

Build the Rust core (rebuilds automatically on change):

```sh
bun run dev:core      # cargo watch on meron-core
```

Run the desktop app in dev mode (Vite HMR for the frontend). The root command
enters [`desktop/`](desktop/) for you:

```sh
bun run dev           # wails3 dev
```

## Build

```sh
bun run build:core    # build the Rust core
bun run build         # build the desktop app
bun run build:release # full release build (scripts/build.sh)
```

### Mac App Store

The store build is sandboxed, universal and signed differently from the DMG:
it ships the sidecar inside the bundle, drops the entitlements the store
rejects, and leaves updates to the store. The header comment in
`scripts/build-mas.sh` covers the required certificates and environment.

```sh
bun scripts/upload-mac-app-store.ts  # builds, then uploads
./scripts/build-mas.sh               # build only -> dist/Meron-mas.pkg
```

Building universal needs both Rust targets, so run it inside `nix-shell`
(which provides `rustup`); `MAS_ARCHS=arm64` opts out at the cost of Intel
support.

### Linux packaging

Package-managed builds should launch Meron with
`MERON_DISABLE_SELF_UPDATE=1`. This disables release polling and self-update
controls so updates remain owned by the package manager.

For standalone tarball and AppImage installations in a root-owned directory
(for example `/opt/meron`), the updater requests administrator authorization
through PolicyKit. This requires `pkexec` and a running desktop PolicyKit
authentication agent. Only the binary replacement runs as root; Meron restarts
as the original user. Cancelling authorization leaves the download available
for another attempt. No custom PolicyKit rules are needed.

## Testing

Run the full suite (Go, Rust, i18n, and frontend):

```sh
bun run test
```

Or individually:

```sh
bun run test:go
bun run test:rust
bun run test:frontend
bun run test:i18n
bun run test:integration   # tagged integration tests against a mail harness
```

## Localization

Translation catalogs live in [`locales/`](locales/). Validate and regenerate
generated message types with:

```sh
bun run i18n:validate
bun run i18n:generate
bun run test:i18n
```
