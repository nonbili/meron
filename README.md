<h1 align="center">Meron</h1>
<div align="center">
  <p>Messages that spark joy</p>
  <img src="desktop/build/appicon.png" width="128" alt="Meron">
</div>

Meron is a fast and secure email app with chat and kanban views.

Install from Microsoft Store, Flathub, Snap Store, Google Play, App Store, or download installers from GitHub.

[<img src="https://img.shields.io/badge/Microsoft%20Store-15508c.svg?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyBlbmFibGUtYmFja2dyb3VuZD0ibmV3IDAgMCAyNDk5LjYgMjUwMCIgdmlld0JveD0iMCAwIDI0OTkuNiAyNTAwIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxwYXRoIGQ9Im0xMTg3LjkgMTE4Ny45aC0xMTg3Ljl2LTExODcuOWgxMTg3Ljl6IiBmaWxsPSIjZjI1MDIyIi8+PHBhdGggZD0ibTI0OTkuNiAxMTg3LjloLTExODh2LTExODcuOWgxMTg3Ljl2MTE4Ny45eiIgZmlsbD0iIzdmYmEwMCIvPjxwYXRoIGQ9Im0xMTg3LjkgMjUwMGgtMTE4Ny45di0xMTg3LjloMTE4Ny45eiIgZmlsbD0iIzAwYTRlZiIvPjxwYXRoIGQ9Im0yNDk5LjYgMjUwMGgtMTE4OHYtMTE4Ny45aDExODcuOXYxMTg3Ljl6IiBmaWxsPSIjZmJiOTAwIi8+PC9zdmc+Cg=="
      alt="Get it on Microsoft Store"
      height="50">](https://apps.microsoft.com/detail/9pcjrrdcwd7x)
[<img src="https://img.shields.io/badge/Flathub-000000?style=for-the-badge&logo=flathub"
      alt="Get it on Flathub"
      hspace="8"
      height="50">](https://flathub.org/apps/jp.nonbili.meron)
[<img src="https://img.shields.io/badge/Snap%20Store-252525?style=for-the-badge&logo=snapcraft"
      alt="Get it on Snap Store"
      height="50">](https://snapcraft.io/meron)
[<img src="https://img.shields.io/badge/App_Store-0D96F6?style=for-the-badge&logo=app-store&logoColor=white"
      alt="Get it on App Store"
      hspace="8"
      height="50">](https://apps.apple.com/us/app/meron-mail/id6782740236)
[<img src="https://img.shields.io/badge/Google_Play-01875f?style=for-the-badge&logo=google-play"
      alt="Get it on Google Play"
      height="50">](https://play.google.com/store/apps/details?id=jp.nonbili.meron)
[<img src="https://img.shields.io/badge/GitHub%20Releases-100000?style=for-the-badge&logo=github"
      alt="Get it on GitHub"
      hspace="8"
      height="50">](https://github.com/nonbili/meron/releases)

<details>
<summary>AppImage notes</summary>

The Linux AppImage and tarball use the system WebKit, so they need
`webkit2gtk-4.1` installed (`libwebkit2gtk-4.1-0` on Debian/Ubuntu,
`webkit2gtk4.1` on Fedora, `webkit2gtk-4.1` on Arch). The Flatpak and snap
bundle their own and need nothing extra.

The AppImage carries update information and ships an accompanying
`meron-linux-amd64.AppImage.zsync`, so AppImageUpdate, AppImageLauncher, AM and
similar tools can update it by downloading only the changed blocks. The in-app
updater works as well and always fetches the full image.

</details>

## Screenshots

| Unified inbox | Kanban board |
| --- | --- |
| ![Unified inbox](screenshots/unified-inbox.png) | ![Kanban board](screenshots/kanban-board.png) |

| Media gallery | Media grid |
| --- | --- |
| ![Media gallery](screenshots/media-gallery.png) | ![Media grid](screenshots/media-grid.png) |

| Themes |
| --- |
| ![Themes](screenshots/themes.png) |

## Features

- Unified Inbox, kanban and chat views
- IMAP/SMTP email and RSS/Atom feeds ([Exchange via DavMail](docs/exchange-davmail.md))
- Threaded conversations with rich-text composing and media galleries
- Easy setup with OAuth or automatic mailbox discovery
- Encrypted local storage with credentials kept in the OS keyring
- Themes and 20+ languages

## Architecture

| Component | Stack | Location |
| --- | --- | --- |
| Core engine | Rust | [`meron-core/`](meron-core/) |
| Desktop app | Go + Wails | [`desktop/`](desktop/) |
| Desktop UI | React + TypeScript + Tailwind | [`desktop/frontend/`](desktop/frontend/) |
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
