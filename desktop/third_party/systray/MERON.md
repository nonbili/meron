# Meron tray patches

This directory contains fyne.io/systray
`v1.12.3-0.20260913152509-528cad23fade` (commit `528cad23fade`), under the
upstream license in LICENSE. The desktop module uses a local Go replacement.
Local changes:

- Unix: `SetIconWithName` replaces the name, theme path, and fallback pixmap
  before emitting NewIcon. `SetIcon(nil)` clears a published pixmap safely.
  Property publication holds the state lock through snapshot/export/assignment
  so an icon update during startup cannot be dropped.
- macOS: `SetTemplateIconWithBadge` updates the template and red badge in one
  native operation. The delegate retains requests before the status button
  exists and applies them when it creates the item. The badge uses a transparent
  gap rather than an opaque ring.
- Windows tests: update menu calls to the upstream parent/shortcut signature,
  use unsigned menu IDs, and keep the required icon fixture in testdata.

These APIs belong to the tray implementation so the application need not
access its private globals or ivars. Preserve the APIs and their regression
tests when updating from upstream.

Checks:

- Unix: `dbus-run-session -- go test . -run TestIconWithName` in this directory.
- macOS: `bash tests/test-tray-badge-darwin.sh` compiles and exercises the actual
  AppKit implementation, including a badge request before item creation,
  transparent cutout, red drawing, read/unread changes, and switching to colour.
