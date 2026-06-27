<h1 align="center">Meron</h1>
<div align="center">
  <p>Messages that spark joy</p>
  <img src="build/appicon.png" width="128" alt="Meron">
</div>

Meron is a cross-platform email and feed client. The desktop app is built with
[Wails](https://wails.io) (Go + React), the mobile apps (Android and iOS) are
built with Kotlin Multiplatform, and all platforms share a single Rust core
([`meron-core`](meron-core/)) that handles IMAP/SMTP/MIME, RSS, OAuth, and
encrypted local storage.

## Screenshots

| Unified inbox | Kanban board |
| --- | --- |
| ![Unified inbox](https://meron.im/screenshots/unified-inbox.png) | ![Kanban board](https://meron.im/screenshots/kanban-board.png) |

| Media gallery | Media grid |
| --- | --- |
| ![Media gallery](https://meron.im/screenshots/media-gallery.png) | ![Media grid](https://meron.im/screenshots/media-grid.png) |

| Themes |
| --- |
| ![Themes](https://meron.im/screenshots/themes.png) |

## Features

- **Email** over IMAP/SMTP, with threaded conversations and a rich-text composer
- **RSS / Atom feeds** alongside your mail
- **OAuth** sign-in (e.g. Gmail) plus password auth with mailbox autodiscovery
- **Encrypted local storage** (SQLite via SQLCipher), with credentials kept in
  the OS keyring
- **Native notifications**, system tray, and `mailto:` handling on desktop
- **Push notifications** on mobile via IMAP IDLE
- **Localized** into 20+ languages (see [`locales/`](locales/))

## Architecture

| Component | Stack | Location |
| --- | --- | --- |
| Core engine | Rust | [`meron-core/`](meron-core/) |
| Desktop app | Go + Wails | root (`*.go`) |
| Desktop UI | React + TypeScript + Tailwind | [`frontend/`](frontend/) |
| Mobile apps | Kotlin Multiplatform (Android/iOS) | [`mobile/`](mobile/) |

The Rust core runs as a sidecar process on desktop (driven over JSON-lines
stdio) and is linked directly into the mobile apps over FFI/JNI. This keeps all
mail, feed, and storage logic in one place across every platform.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for prerequisites and instructions on
building, testing, and translating Meron.

## License

Meron is licensed under the [GNU Affero General Public License v3.0](LICENSE).

Copyright © 2026 Nonbili Inc.
