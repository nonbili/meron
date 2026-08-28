# Contributing to Meron

## Prerequisites

- [Go](https://go.dev) 1.23+
- [Rust](https://www.rust-lang.org) (with `cargo`)
- [Bun](https://bun.sh)
- The [Wails CLI](https://wails.io): `go install github.com/wailsapp/wails/v2/cmd/wails@latest`
- SQLite, and on Linux: GTK 3 + WebKitGTK 4.1

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
bun run dev           # wails dev
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
bun run i18n:check
```
