package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseThreadListResponse
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

internal fun MeronMobileState.importOpmlFile(picked: PickedFile?) {
    if (picked != null) {
        runCatching { picked.bytes.decodeToString() }
            .onSuccess { opml ->
                val accountId = selectedCoreAccountId
                if (accountId == UNIFIED_ACCOUNT_ID || accountId.isBlank()) {
                    status = trs("mobile.status.selectRssAccountFirst")
                    return@onSuccess
                }
                if (!coreLoaded) {
                    status = coreUnavailableMessage
                    return@onSuccess
                }
                scope.launchThreadListRead(threadRemovalGuard) { read ->
                    runCatching {
                        withContext(ioDispatcher) {
                            val client = MobileMailCommandClient(core)
                            val importJson = client.importOpml(ImportOpmlParams(accountId = accountId, opml = opml))
                            client.syncRss(SyncRssParams(accountId = accountId))
                            val foldersJson = client.listFolders(FolderListParams(accountId = accountId))
                            val threadsJson =
                                client.listThreads(
                                    ThreadListParams(
                                        accountId = accountId,
                                        folderId = INBOX_FOLDER,
                                        query = mailSearch.trim(),
                                        filter = mailFilter.protocolValue(),
                                        attachments = mailAttachmentsOnly,
                                    ),
                                )
                            Triple(importJson, foldersJson, threadsJson)
                        }
                    }.onSuccess { (importJson, foldersJson, threadsJson) ->
                        val imported = parseOpmlImportCountResponse(importJson)
                        coreFolders = parseFolderListResponse(foldersJson)
                        coreThreads = withLocalDraftFlags(read.filter(parseThreadListResponse(threadsJson)))
                        selectedCoreFolder = INBOX_FOLDER
                        visibleMailboxKey =
                            mailboxCacheKey(accountId, INBOX_FOLDER, mailSearch, mailFilter, mailAttachmentsOnly)
                        mailboxPageDepth = MAILBOX_PAGE_SIZE
                        status = if (imported == 0) trs("feeds.toast.noNewFeedsToImport") else trs("feeds.toast.importedFeeds", mapOf("count" to imported))
                    }.onFailure {
                        status = trsError("mobile.status.opmlImportFailed", it)
                    }
                }
            }.onFailure {
                status = trsError("mobile.status.opmlReadFailed", it)
            }
    }
}

// Restore starts at the file picker; the core decides whether what came
// back needs a passphrase.
internal fun MeronMobileState.pickBackupFileToRestore() {
    services.pickFile(listOf("application/json", "text/plain", "*/*")) { picked ->
        if (picked != null) {
            runCatching { picked.bytes.decodeToString() }
                .onSuccess { document -> importBackup(document) }
                .onFailure { status = trsError("settings.backup.restoreFailed", it) }
        }
    }
}

internal fun MeronMobileState.pickDraftAttachment(onPicked: (DraftAttachment) -> Unit) {
    services.pickFile(listOf("*/*")) { picked ->
        if (picked != null) {
            runCatching { picked.toDraftAttachment() }
                .onSuccess {
                    onPicked(it)
                    status = trs("mobile.status.attachedFile", mapOf("name" to it.displayName))
                }.onFailure {
                    status = trsError("mobile.status.attachFailed", it)
                }
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
internal fun MeronMobileState.pickAccountMediaFile() {
    val target = accountMediaUploadTarget
    accountMediaUploadTarget = null
    if (target != null) {
        services.pickImage { picked ->
            if (picked == null) return@pickImage
            runCatching {
                AccountMediaFileParams(
                    accountId = target.account.id,
                    filename = picked.name,
                    mime = picked.mimeType.ifBlank { "application/octet-stream" },
                    data = Base64.Default.encode(picked.bytes),
                )
            }.onSuccess { params ->
                if (!coreLoaded) {
                    status = coreUnavailableMessage
                    return@onSuccess
                }
                scope.launch {
                    runCatching {
                        withContext(ioDispatcher) {
                            val client = MobileMailCommandClient(core)
                            val uploadJson =
                                if (target.wallpaper) {
                                    client.writeAccountChatWallpaperFile(params)
                                } else {
                                    client.writeAccountAvatarFile(params)
                                }
                            val mediaUrl = parseMediaFileUrlResponse(uploadJson)
                            if (mediaUrl.isBlank()) error("Media upload returned no URL")
                            if (target.wallpaper) {
                                client.setAccountChatWallpaper(AccountChatWallpaperParams(target.account.id, customUrl = mediaUrl))
                            } else {
                                client.setAccountAvatar(AccountAvatarParams(target.account.id, mediaUrl))
                            }
                            client.listAccounts()
                        }
                    }.onSuccess {
                        accountJson = it
                        coreAccounts = parseAccountListResponse(it)
                        status = trs(if (target.wallpaper) "mobile.status.chatWallpaperUpdated" else "mobile.status.avatarUpdated")
                    }.onFailure {
                        status = trsError(if (target.wallpaper) "wallpaper.uploadFailed" else "settings.toast.failedToUploadAvatar", it)
                    }
                }
            }.onFailure {
                status = trsError("mobile.status.mediaReadFailed", it)
            }
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
internal fun MeronMobileState.pickKanbanBoardMediaFile() {
    val target = kanbanBoardMediaTarget
    kanbanBoardMediaTarget = null
    if (target != null) {
        services.pickImage { picked ->
            if (picked == null) return@pickImage
            val mediaUrl = "data:${picked.mimeType.ifBlank { "application/octet-stream" }};base64,${Base64.Default.encode(picked.bytes)}"
            if (target.wallpaper) {
                updateKanbanBoard(
                    target.board.id,
                    target.board.name,
                    target.board.avatarUrl,
                    "",
                    mediaUrl,
                )
                status = trs("mobile.status.boardWallpaperUpdated")
            } else {
                updateKanbanBoard(
                    target.board.id,
                    target.board.name,
                    mediaUrl,
                    target.board.wallpaperPresetId,
                    target.board.wallpaperUrl,
                )
                status = trs("mobile.status.boardAvatarUpdated")
            }
        }
    }
}

/** Saves what the platform's file-save launchers were asked to write. */
internal fun MeronMobileState.installFileSaveLaunchers() {
    launchOpmlExport = { fileName ->
        val document = pendingOpmlExport
        services.saveFile(document.encodeToByteArray(), fileName, "text/xml") { result ->
            result
                .onSuccess { saved ->
                    if (saved) {
                        pendingOpmlExport = ""
                        status = trs("feeds.toast.feedsExported")
                    }
                }.onFailure { status = trsError("mobile.status.opmlExportFailed", it) }
        }
    }
    launchBackupExport = { fileName ->
        val document = pendingBackupExport
        pendingBackupExport = ""
        services.saveFile(document.encodeToByteArray(), fileName, "application/json") { result ->
            result
                .onSuccess { saved ->
                    if (saved) status = trs("settings.backup.exported")
                }.onFailure { status = trsError("mobile.status.backupExportFailed", it) }
        }
    }
    launchAttachmentSave = { fileName ->
        val attachment = pendingAttachmentSave
        pendingAttachmentSave = null
        if (attachment != null) {
            scope.launch {
                runCatching {
                    withContext(ioDispatcher) { readAttachmentBytes(attachment) }
                }.onSuccess { bytes ->
                    val resolved = normalizedRemoteImage(attachment, bytes)
                    services.saveFile(bytes, safeAttachmentFilename(resolved), resolved.mimeType.ifBlank { "application/octet-stream" }) { result ->
                        result
                            .onSuccess { saved ->
                                if (saved) status = trs("chat.savedAttachment", mapOf("filename" to attachment.filename.ifBlank { fileName }))
                            }.onFailure { status = trsError("mobile.status.attachmentSaveFailed", it) }
                    }
                }.onFailure {
                    status = trsError("mobile.status.attachmentSaveFailed", it)
                }
            }
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
private fun PickedFile.toDraftAttachment(): DraftAttachment =
    DraftAttachment(
        id = name,
        displayName = name.ifBlank { "attachment" },
        mimeType = mimeType.ifBlank { "application/octet-stream" },
        sizeBytes = bytes.size.toLong(),
        dataBase64 = Base64.Default.encode(bytes),
    )
