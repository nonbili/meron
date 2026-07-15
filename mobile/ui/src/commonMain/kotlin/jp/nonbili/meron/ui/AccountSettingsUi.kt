package jp.nonbili.meron.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.NotificationsOff
import androidx.compose.material.icons.filled.PauseCircle
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import kotlinx.coroutines.withContext

@Composable
internal fun SettingsAccountDetailPage(
    account: AccountSummary,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    showInNavigation: Boolean,
    onSave: (
        displayName: String,
        senderName: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        loadRemoteImages: Boolean,
        conversationHtml: Boolean,
        includedInUnified: Boolean,
        showInNavigation: Boolean,
        muted: Boolean,
        paused: Boolean,
        rssSyncIntervalMinutes: Int,
        aliasesText: String,
    ) -> Unit,
    onPickAvatar: () -> Unit,
    onOpenWallpaper: () -> Unit,
    onMoveUp: () -> Unit,
    onMoveDown: () -> Unit,
    onRemove: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val isRss = accountSummaryIsRss(account)
    var displayName by remember(account.id) { mutableStateOf(account.displayName) }
    var senderName by remember(account.id) { mutableStateOf(account.senderName) }
    var avatarUrl by remember(account.id) { mutableStateOf(account.avatarUrl) }
    var wallpaperPresetId by remember(account.id) { mutableStateOf(account.chatWallpaperPresetId) }
    var wallpaperCustomUrl by remember(account.id, account.chatWallpaperKind, account.chatWallpaperUrl) {
        mutableStateOf(if (account.chatWallpaperKind == "custom") account.chatWallpaperUrl else "")
    }
    var loadRemoteImages by remember(account.id) { mutableStateOf(account.loadRemoteImages || isRss) }
    var conversationHtml by remember(account.id) { mutableStateOf(account.conversationHtml) }
    var includedInUnified by remember(account.id) { mutableStateOf(account.includedInUnified) }
    var visibleInNavigation by remember(account.id, showInNavigation) { mutableStateOf(showInNavigation) }
    var muted by remember(account.id) { mutableStateOf(account.muted) }
    var paused by remember(account.id) { mutableStateOf(account.paused) }
    var intervalText by remember(account.id) { mutableStateOf(account.rssSyncIntervalMinutes.toString()) }
    var aliasEntries by remember(account.id) {
        mutableStateOf(account.aliases.map { it.email to it.name })
    }
    var confirmRemove by remember(account.id) { mutableStateOf(false) }
    val accountTitle =
        if (isRss) {
            displayName.ifBlank { tr("accounts.rssAtomFeeds") }
        } else {
            displayName.ifBlank { account.email.ifBlank { account.id } }
        }
    val accountSubtitle = if (isRss) "" else account.email.ifBlank { account.id }

    // Autosave: every control persists immediately, matching the desktop panel.
    // The lambda reads the live state values at call time, so flipping a toggle
    // then calling persist() writes the just-updated value.
    val persist = {
        onSave(
            displayName,
            senderName,
            avatarUrl,
            wallpaperPresetId,
            loadRemoteImages,
            conversationHtml,
            includedInUnified,
            visibleInNavigation,
            muted,
            paused,
            intervalText.toIntOrNull()?.coerceIn(5, 1440) ?: account.rssSyncIntervalMinutes.coerceIn(5, 1440),
            aliasEntries
                .filter { it.first.isNotBlank() }
                .joinToString("\n") { (email, name) ->
                    if (name.isBlank()) email else "$email, $name"
                },
        )
    }

    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text(tr("settings.account.removeAccountTitle")) },
            text = { Text(trf("settings.account.removeAccountText", accountTitle)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmRemove = false
                    onRemove()
                }) {
                    Text(tr("settings.account.removeAccount"), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmRemove = false }) { Text(tr("buttons.cancel")) }
            },
        )
    }
    LazyColumn(modifier) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth().padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                AccountAvatarEditor(
                    name = accountTitle,
                    url = avatarUrl,
                    onPick = onPickAvatar,
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        accountTitle,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (accountSubtitle.isNotBlank()) {
                        Text(
                            accountSubtitle,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }

        item { SettingsSectionLabel(tr("settings.account.profile")) }
        item {
            SettingsTextRow(
                value = displayName,
                label = tr("accounts.fields.accountName"),
                onValueChange = {
                    displayName = it
                    persist()
                },
            )
        }
        if (!isRss) {
            item {
                SettingsTextRow(
                    value = senderName,
                    label = tr("settings.account.senderName"),
                    onValueChange = {
                        senderName = it
                        persist()
                    },
                )
            }
        }
        item { SettingsSectionLabel(tr("settings.account.chatBackground")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Image,
                title = tr("settings.account.chatBackground"),
                subtitle =
                    if (wallpaperCustomUrl.isNotBlank()) {
                        tr("wallpaper.customBackground")
                    } else {
                        wallpaperPresetDisplayName(wallpaperPresetId)
                    },
                onClick = onOpenWallpaper,
                trailing = {
                    WallpaperRowPreview(
                        presetId = wallpaperPresetId,
                        customUrl = wallpaperCustomUrl,
                    )
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.account.visibility")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Inbox,
                title = tr("settings.account.showInUnifiedInbox"),
                subtitle = null,
                checked = includedInUnified,
            ) {
                includedInUnified = !includedInUnified
                persist()
            }
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Visibility,
                title = tr("settings.account.showInSideNav"),
                subtitle = null,
                checked = visibleInNavigation,
            ) {
                visibleInNavigation = !visibleInNavigation
                persist()
            }
        }

        item { SettingsSectionLabel(tr("settings.account.notificationsSync")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.NotificationsOff,
                title = tr("settings.account.muteNotifications"),
                subtitle = null,
                checked = muted,
            ) {
                muted = !muted
                persist()
            }
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.PauseCircle,
                title = tr("settings.account.pauseAccount"),
                subtitle = null,
                checked = paused,
            ) {
                paused = !paused
                persist()
            }
        }
        if (isRss) {
            item {
                SettingsTextRow(
                    value = intervalText,
                    label = tr("settings.account.syncIntervalMinutes"),
                    supporting = tr("settings.account.syncIntervalRange"),
                    keyboardDigits = true,
                    onValueChange = {
                        intervalText = it.filter(Char::isDigit).take(4)
                        persist()
                    },
                )
            }
        }

        item { SettingsSectionLabel(tr("settings.account.content")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Image,
                title = tr("settings.account.loadRemoteImages"),
                subtitle = null,
                checked = loadRemoteImages,
            ) {
                loadRemoteImages = !loadRemoteImages
                persist()
            }
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Code,
                title = tr("settings.account.renderHtmlMessages"),
                subtitle = null,
                checked = conversationHtml,
            ) {
                conversationHtml = !conversationHtml
                persist()
            }
        }

        if (!isRss) {
            item { SettingsSectionLabel(tr("settings.account.aliases")) }
            itemsIndexed(aliasEntries) { index, (email, name) ->
                AliasEditorRow(
                    email = email,
                    name = name,
                    onEmailChange = { value ->
                        aliasEntries = aliasEntries.toMutableList().also { it[index] = value to it[index].second }
                        persist()
                    },
                    onNameChange = { value ->
                        aliasEntries = aliasEntries.toMutableList().also { it[index] = it[index].first to value }
                        persist()
                    },
                    onRemove = {
                        aliasEntries = aliasEntries.toMutableList().also { it.removeAt(index) }
                        persist()
                    },
                )
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.Add,
                    title = tr("settings.account.addAlias"),
                    subtitle = tr("settings.account.addAliasHint"),
                    onClick = { aliasEntries = aliasEntries + ("" to "") },
                )
            }
        }

        if (canMoveUp || canMoveDown) {
            item { SettingsSectionLabel(tr("settings.account.order")) }
            if (canMoveUp) {
                item {
                    SettingsRow(
                        icon = Icons.Filled.KeyboardArrowUp,
                        title = tr("mobile.accounts.moveUp"),
                        subtitle = null,
                        onClick = onMoveUp,
                    )
                }
            }
            if (canMoveDown) {
                item {
                    SettingsRow(
                        icon = Icons.Filled.KeyboardArrowDown,
                        title = tr("mobile.accounts.moveDown"),
                        subtitle = null,
                        onClick = onMoveDown,
                    )
                }
            }
        }

        item { SettingsSectionLabel(tr("settings.dangerZone")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Delete,
                title = tr("settings.account.removeAccount"),
                subtitle = tr("settings.account.deleteCachedMailHint"),
                onClick = { confirmRemove = true },
                destructive = true,
            )
        }
        item { Spacer(Modifier.height(24.dp)) }
    }
}

// A single send-as alias rendered as an editable row instead of a free-form
// textarea: an email field, an optional display-name field, and a remove button.
@Composable
internal fun AliasEditorRow(
    email: String,
    name: String,
    onEmailChange: (String) -> Unit,
    onNameChange: (String) -> Unit,
    onRemove: () -> Unit,
) {
    Surface(color = MaterialTheme.colorScheme.surface, modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedTextField(
                        value = email,
                        onValueChange = onEmailChange,
                        label = { Text(tr("accounts.fields.emailAddress")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Email),
                        modifier = Modifier.weight(1f),
                    )
                    if (email.isEmpty()) {
                        val clipboardManager = LocalClipboardManager.current
                        IconButton(
                            onClick = {
                                val text = clipboardManager.getText()?.text.orEmpty()
                                if (text.isNotEmpty()) {
                                    onEmailChange(text)
                                }
                            },
                        ) {
                            Icon(
                                Icons.Filled.ContentPaste,
                                contentDescription = "Paste",
                            )
                        }
                    }
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = onNameChange,
                        label = { Text(tr("accounts.fields.displayNameMeronOnly")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions,
                        modifier = Modifier.weight(1f),
                    )
                    if (name.isEmpty()) {
                        val clipboardManager = LocalClipboardManager.current
                        IconButton(
                            onClick = {
                                val text = clipboardManager.getText()?.text.orEmpty()
                                if (text.isNotEmpty()) {
                                    onNameChange(text)
                                }
                            },
                        ) {
                            Icon(
                                Icons.Filled.ContentPaste,
                                contentDescription = "Paste",
                            )
                        }
                    }
                }
            }
            IconButton(onClick = onRemove) {
                Icon(
                    Icons.Filled.Close,
                    contentDescription = tr("settings.account.removeAlias"),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
internal fun AccountAvatarEditor(
    name: String,
    url: String,
    onPick: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    var image by remember(url) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(url) {
        image = null
        if (url.isNotBlank()) {
            image = withContext(ioDispatcher) { loadImageBitmapRef(url) }
        }
    }
    Box(
        modifier =
            modifier
                .size(64.dp)
                .clip(CircleShape)
                .then(if (onPick != null) Modifier.clickable(onClick = onPick) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        val bmp = image
        if (bmp != null) {
            Image(
                bitmap = bmp,
                contentDescription = tr("avatar.edit"),
                modifier = Modifier.size(64.dp).clip(CircleShape),
                contentScale = ContentScale.Crop,
            )
        } else {
            Avatar(name, 64.dp)
        }
        if (onPick != null) {
            Box(
                modifier =
                    Modifier
                        .align(Alignment.BottomEnd)
                        .size(22.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Filled.Edit,
                    contentDescription = tr("settings.account.changeAvatar"),
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(13.dp),
                )
            }
        }
    }
}
