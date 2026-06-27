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

Run the desktop app in dev mode (Vite HMR for the frontend):

```sh
bun run dev           # wails dev
```

## Build

```sh
bun run build:core    # build the Rust core
bun run build         # build the desktop app
bun run build:release # full release build (scripts/build.sh)
```

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
