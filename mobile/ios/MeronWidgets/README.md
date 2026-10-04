# iOS unread widgets

Meron provides a small unread-count widget and medium/large unread-list widgets.
Long-press a widget and choose Edit Widget to select Unified or an account. Tap a
list row to open its thread; tap the count or header to open Meron.

The app publishes local unread counts and up to eight thread summaries to an
atomic JSON snapshot in `group.jp.nonbili.meron`. Widgets never open the mail
database or make network requests. Snapshots update after mailbox mutations,
foreground sync events, and background refresh. The displayed update time refers
to the snapshot; delivery of new mail still depends on app sync. WidgetKit decides
when reload requests are rendered.

Unlike the SQLCipher mail database, this snapshot is plaintext JSON. It contains
account names, unread counts, and up to eight senders and subjects per scope,
plus thread identifiers and dates. iOS file protection keeps it inaccessible
until the first device unlock after boot; it remains accessible while subsequently
locked so widgets can refresh. This is the privacy trade-off for displaying mail
previews without giving the extension access to the encrypted database or its key.
The views mark counts and message previews as privacy-sensitive for system
redaction.

For device builds, register the `group.jp.nonbili.meron` App Group and enable it
for both `jp.nonbili.meron` and `jp.nonbili.meron.widgets` in the Apple Developer
account. Both targets include the entitlement and use automatic signing.
Launch Meron once before adding its widgets from the widget gallery.

`MeronLocalization` generates the app catalog and a separate nine-key widget
catalog before their respective targets compile them. This dependency also supports building the widget target
by itself, without creating a dependency cycle through the containing app.
