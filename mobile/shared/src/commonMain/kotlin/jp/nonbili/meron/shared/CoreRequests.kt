package jp.nonbili.meron.shared

fun accountListRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.AccountList)

fun accountAddPasswordRequest(
    id: Long = 1,
    params: AddPasswordAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAddPassword, params.toJson())

fun accountAutodiscoverRequest(
    id: Long = 1,
    params: AutodiscoverAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAutodiscover, params.toJson())

fun accountAddOAuthRequest(
    id: Long = 1,
    params: AddOAuthAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAddOAuth, params.toJson())

fun accountExchangeOAuthCodeRequest(
    id: Long = 1,
    params: ExchangeOAuthCodeParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountExchangeOAuthCode, params.toJson())

fun accountAddRssRequest(
    id: Long = 1,
    params: AddRssAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAddRss, params.toJson())

fun accountRemoveRequest(
    id: Long = 1,
    params: AccountIdParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountRemove, params.toJson())

fun accountSetNameRequest(
    id: Long = 1,
    params: AccountNameParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetName, params.toJson())

fun accountSetSenderNameRequest(
    id: Long = 1,
    params: AccountNameParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetSenderName, params.toJson())

fun accountSetAvatarRequest(
    id: Long = 1,
    params: AccountAvatarParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetAvatar, params.toJson())

fun accountWriteAvatarFileRequest(
    id: Long = 1,
    params: AccountMediaFileParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountWriteAvatarFile, params.toJson())

fun accountSetChatWallpaperRequest(
    id: Long = 1,
    params: AccountChatWallpaperParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetChatWallpaper, params.toJson())

fun accountWriteChatWallpaperFileRequest(
    id: Long = 1,
    params: AccountMediaFileParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountWriteChatWallpaperFile, params.toJson())

fun accountSetImagesRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetImages, params.toJson())

fun accountSetConversationHtmlRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetConversationHtml, params.toJson())

fun accountSetUnifiedRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetUnified, params.toJson())

fun accountSetMutedRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetMuted, params.toJson())

fun accountSetPausedRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetPaused, params.toJson())

fun accountSetRssSyncIntervalRequest(
    id: Long = 1,
    params: AccountRssSyncIntervalParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetRssSyncInterval, params.toJson())

fun accountSetAliasesRequest(
    id: Long = 1,
    params: AccountAliasesParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetAliases, params.toJson())

fun accountReorderRequest(
    id: Long = 1,
    params: AccountReorderParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountReorder, params.toJson())

fun feedAddRequest(
    id: Long = 1,
    params: AddRssFeedParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedAdd, params.toJson())

fun feedRemoveRequest(
    id: Long = 1,
    params: RemoveRssFeedParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedRemove, params.toJson())

fun feedMoveRequest(
    id: Long = 1,
    params: MoveRssFeedParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedMove, params.toJson())

fun feedExportOpmlRequest(
    id: Long = 1,
    params: ExportOpmlParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedExportOpml, params.toJson())

fun feedImportOpmlRequest(
    id: Long = 1,
    params: ImportOpmlParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedImportOpml, params.toJson())

fun appPrefsGetRequest(
    id: Long = 1,
    params: AppPrefsGetParams,
): CoreRequest = CoreRequest(id, MobileCommand.AppPrefsGet, params.toJson())

fun appPrefsSetRequest(
    id: Long = 1,
    params: AppPrefsSetParams,
): CoreRequest = CoreRequest(id, MobileCommand.AppPrefsSet, params.toJson())

fun backupExportRequest(
    id: Long = 1,
    params: ExportBackupParams,
): CoreRequest = CoreRequest(id, MobileCommand.BackupExport, params.toJson())

fun backupImportRequest(
    id: Long = 1,
    params: ImportBackupParams,
): CoreRequest = CoreRequest(id, MobileCommand.BackupImport, params.toJson())

fun folderListRequest(
    id: Long = 1,
    params: FolderListParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderList, params.toJson())

fun folderCreateRequest(
    id: Long = 1,
    params: FolderCreateParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderCreate, params.toJson())

fun folderDeleteRequest(
    id: Long = 1,
    params: FolderDeleteParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderDelete, params.toJson())

fun folderRenameRequest(
    id: Long = 1,
    params: FolderRenameParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderRename, params.toJson())

fun contactSuggestRequest(
    id: Long = 1,
    params: ContactSuggestParams,
): CoreRequest = CoreRequest(id, MobileCommand.ContactSuggest, params.toJson())

fun contactThreadsRequest(
    id: Long = 1,
    params: ContactThreadsParams,
): CoreRequest = CoreRequest(id, MobileCommand.ContactThreads, params.toJson())

fun threadListRequest(
    id: Long = 1,
    params: ThreadListParams,
): CoreRequest = CoreRequest(id, MobileCommand.ThreadList, params.toJson())

fun starredItemsRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.StarredItems)

fun threadReadRequest(
    id: Long = 1,
    params: ThreadReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.ThreadRead, params.toJson())

fun attachmentReadRequest(
    id: Long = 1,
    params: AttachmentReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.AttachmentRead, params.toJson())

fun setAccountProxyRequest(
    id: Long = 1,
    params: AccountProxyParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetProxy, params.toJson())

fun probeCertRequest(
    id: Long = 1,
    params: ProbeCertParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountProbeCert, params.toJson())

fun setAccountCertPinRequest(
    id: Long = 1,
    params: AccountCertPinParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetCertPin, params.toJson())

fun setAccountSignatureRequest(
    id: Long = 1,
    params: AccountSignatureParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetSignature, params.toJson())

fun proxyGetRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.AppProxyGet)

fun proxySetRequest(
    id: Long = 1,
    params: ProxyParams,
): CoreRequest = CoreRequest(id, MobileCommand.AppProxySet, params.toJson())

fun storageUsageRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.StorageUsage)

fun storageClearCacheRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.StorageClearCache)

fun syncMailRequest(
    id: Long = 1,
    params: SyncMailParams,
): CoreRequest = CoreRequest(id, MobileCommand.Sync, params.toJson())

fun syncRssRequest(
    id: Long = 1,
    params: SyncRssParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssSync, params.toJson())

fun rssThreadRequest(
    id: Long = 1,
    params: RssThreadParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssThread, params.toJson())

fun sendMailRequest(
    id: Long = 1,
    params: SendMailParams,
): CoreRequest = CoreRequest(id, MobileCommand.Send, params.toJson())

fun saveDraftRequest(
    id: Long = 1,
    params: SaveDraftParams,
): CoreRequest = CoreRequest(id, MobileCommand.SaveDraft, params.toJson())

fun discardDraftRequest(
    id: Long = 1,
    params: DiscardDraftParams,
): CoreRequest = CoreRequest(id, MobileCommand.DiscardDraft, params.toJson())

fun archiveThreadRequest(
    id: Long = 1,
    params: ThreadActionParams,
): CoreRequest = CoreRequest(id, MobileCommand.Archive, params.archiveJson())

fun moveThreadRequest(
    id: Long = 1,
    params: MoveThreadParams,
): CoreRequest = CoreRequest(id, MobileCommand.Move, params.toJson())

fun copyThreadRequest(
    id: Long = 1,
    params: CopyThreadParams,
): CoreRequest = CoreRequest(id, MobileCommand.Copy, params.toJson())

fun deleteThreadRequest(
    id: Long = 1,
    params: ThreadActionParams,
): CoreRequest = CoreRequest(id, MobileCommand.Delete, params.deleteJson())

fun markReadRequest(
    id: Long = 1,
    params: MarkReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.MarkRead, params.toJson())

fun markAllReadRequest(
    id: Long = 1,
    params: MarkAllReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.MarkAllRead, params.toJson())

fun emptyFolderRequest(
    id: Long = 1,
    params: EmptyFolderParams,
): CoreRequest = CoreRequest(id, MobileCommand.EmptyFolder, params.toJson())

fun markStarredRequest(
    id: Long = 1,
    params: MarkStarredParams,
): CoreRequest = CoreRequest(id, MobileCommand.MarkStarred, params.toJson())

fun rssMarkReadRequest(
    id: Long = 1,
    params: RssMarkReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssMarkRead, params.toJson())

fun rssMarkStarredRequest(
    id: Long = 1,
    params: RssMarkStarredParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssMarkStarred, params.toJson())
