<h1 align="center">Meron</h1>
<div align="center">
  <p>Messages that spark joy</p>
  <img src="build/appicon.png" width="128" alt="Meron">
</div>

Meron is a fast/secure email app with chat and kanban view.

Install from Microsoft Store, Snap Store, Google Play, App Store, or download installers from GitHub.

[<img src="https://img.shields.io/badge/Microsoft%20Store-15508c.svg?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyBlbmFibGUtYmFja2dyb3VuZD0ibmV3IDAgMCAyNDk5LjYgMjUwMCIgdmlld0JveD0iMCAwIDI0OTkuNiAyNTAwIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxwYXRoIGQ9Im0xMTg3LjkgMTE4Ny45aC0xMTg3Ljl2LTExODcuOWgxMTg3Ljl6IiBmaWxsPSIjZjI1MDIyIi8+PHBhdGggZD0ibTI0OTkuNiAxMTg3LjloLTExODh2LTExODcuOWgxMTg3Ljl2MTE4Ny45eiIgZmlsbD0iIzdmYmEwMCIvPjxwYXRoIGQ9Im0xMTg3LjkgMjUwMGgtMTE4Ny45di0xMTg3LjloMTE4Ny45eiIgZmlsbD0iIzAwYTRlZiIvPjxwYXRoIGQ9Im0yNDk5LjYgMjUwMGgtMTE4OHYtMTE4Ny45aDExODcuOXYxMTg3Ljl6IiBmaWxsPSIjZmJiOTAwIi8+PC9zdmc+Cg=="
      alt="Get it on Microsoft Store"
      height="50">](https://apps.microsoft.com/detail/9pcjrrdcwd7x)
[<img src="https://img.shields.io/badge/Snap%20Store-252525?style=for-the-badge&logo=snapcraft"
      alt="Get it on Snap Store"
      hspace="16"
      height="50">](https://snapcraft.io/meron)
[<img src="https://img.shields.io/badge/App_Store-0D96F6?style=for-the-badge&logo=app-store&logoColor=white"
      alt="Get it on App Store"
      height="50">](https://apps.apple.com/us/app/meron-mail/id6782740236)
[<img src="https://img.shields.io/badge/Google_Play-01875f?style=for-the-badge&logo=google-play"
      alt="Get it on Google Play"
      hspace="16"
      height="50">](https://play.google.com/store/apps/details?id=jp.nonbili.meron)
[<img src="https://img.shields.io/badge/GitHub%20Releases-100000?style=for-the-badge&logo=github"
      alt="Get it on GitHub"
      height="50">](https://github.com/nonbili/meron/releases)

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
