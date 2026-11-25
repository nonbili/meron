package jp.nonbili.meron.shared

class MobileMailCommandClient(
    private val core: MeronCore,
) {
    suspend fun assertProtocol() {
        requireProtocolVersion(core.protocolVersion())
    }

    suspend fun listAccounts(): String = core.invoke(MobileCommand.AccountList)

    suspend fun addPasswordAccount(params: AddPasswordAccountParams): String = core.invoke(MobileCommand.AccountAddPassword, params.toJson())

    suspend fun autodiscoverAccount(params: AutodiscoverAccountParams): String = core.invoke(MobileCommand.AccountAutodiscover, params.toJson())

    suspend fun addOAuthAccount(params: AddOAuthAccountParams): String = core.invoke(MobileCommand.AccountAddOAuth, params.toJson())

    suspend fun oauthAuthorizeUrl(params: OAuthAuthorizeUrlParams): String = parseOAuthAuthorizeUrlResponse(core.invoke(MobileCommand.OAuthAuthorizeUrl, params.toJson()))

    suspend fun exchangeOAuthCode(params: ExchangeOAuthCodeParams): String = core.invoke(MobileCommand.AccountExchangeOAuthCode, params.toJson())

    suspend fun updateOAuthToken(params: UpdateOAuthTokenParams): String = core.invoke(MobileCommand.AccountUpdateOAuthToken, params.toJson())

    suspend fun addRssAccount(params: AddRssAccountParams): String = core.invoke(MobileCommand.AccountAddRss, params.toJson())

    suspend fun removeAccount(params: AccountIdParams): String = core.invoke(MobileCommand.AccountRemove, params.toJson())

    suspend fun setAccountName(params: AccountNameParams): String = core.invoke(MobileCommand.AccountSetName, params.toJson())

    suspend fun setAccountSenderName(params: AccountNameParams): String = core.invoke(MobileCommand.AccountSetSenderName, params.toJson())

    /** [sizePx] is the pixel size the image is drawn at, so Gravatar returns
     *  a picture that is sharp on the screen's density; core clamps it. */
    suspend fun resolveSenderImage(
        email: String,
        sizePx: Int = 96,
    ): String = core.invoke(MobileCommand.AvatarResolve, jsonObject("email" to email.jsonString(), "size" to sizePx.toString()))

    suspend fun setAccountAvatar(params: AccountAvatarParams): String = core.invoke(MobileCommand.AccountSetAvatar, params.toJson())

    suspend fun writeAccountAvatarFile(params: AccountMediaFileParams): String = core.invoke(MobileCommand.AccountWriteAvatarFile, params.toJson())

    suspend fun setAccountChatWallpaper(params: AccountChatWallpaperParams): String = core.invoke(MobileCommand.AccountSetChatWallpaper, params.toJson())

    suspend fun setAccountColor(params: AccountColorParams): String = core.invoke(MobileCommand.AccountSetColor, params.toJson())

    suspend fun writeAccountChatWallpaperFile(params: AccountMediaFileParams): String = core.invoke(MobileCommand.AccountWriteChatWallpaperFile, params.toJson())

    suspend fun setAccountImages(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetImages, params.toJson())

    suspend fun setAccountConversationHtml(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetConversationHtml, params.toJson())

    suspend fun setAccountUnified(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetUnified, params.toJson())

    suspend fun setAccountMuted(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetMuted, params.toJson())

    suspend fun setAccountPaused(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetPaused, params.toJson())

    suspend fun setAccountSaveSentCopy(params: AccountSaveSentCopyParams): String = core.invoke(MobileCommand.AccountSetSaveSentCopy, params.toJson())

    suspend fun setAccountRssSyncInterval(params: AccountRssSyncIntervalParams): String = core.invoke(MobileCommand.AccountSetRssSyncInterval, params.toJson())

    suspend fun setAccountAliases(params: AccountAliasesParams): String = core.invoke(MobileCommand.AccountSetAliases, params.toJson())

    suspend fun setAccountProxy(params: AccountProxyParams): String = core.invoke(MobileCommand.AccountSetProxy, params.toJson())

    suspend fun probeCert(params: ProbeCertParams): String = core.invoke(MobileCommand.AccountProbeCert, params.toJson())

    suspend fun setAccountCertPin(params: AccountCertPinParams): String = core.invoke(MobileCommand.AccountSetCertPin, params.toJson())

    suspend fun setAccountSignature(params: AccountSignatureParams): String = core.invoke(MobileCommand.AccountSetSignature, params.toJson())

    suspend fun getProxy(): String = core.invoke(MobileCommand.AppProxyGet)

    suspend fun getPrefs(params: AppPrefsGetParams): String = core.invoke(MobileCommand.AppPrefsGet, params.toJson())

    suspend fun setPref(params: AppPrefsSetParams): String = core.invoke(MobileCommand.AppPrefsSet, params.toJson())

    suspend fun setProxy(params: ProxyParams): String = core.invoke(MobileCommand.AppProxySet, params.toJson())

    suspend fun reorderAccounts(params: AccountReorderParams): String = core.invoke(MobileCommand.AccountReorder, params.toJson())

    suspend fun addRssFeed(params: AddRssFeedParams): String = core.invoke(MobileCommand.FeedAdd, params.toJson())

    suspend fun removeRssFeed(params: RemoveRssFeedParams): String = core.invoke(MobileCommand.FeedRemove, params.toJson())

    suspend fun moveRssFeed(params: MoveRssFeedParams): String = core.invoke(MobileCommand.FeedMove, params.toJson())

    suspend fun renameRssFeed(params: RenameRssFeedParams): String = core.invoke(MobileCommand.FeedRename, params.toJson())

    suspend fun exportOpml(params: ExportOpmlParams): String = core.invoke(MobileCommand.FeedExportOpml, params.toJson())

    suspend fun importOpml(params: ImportOpmlParams): String = core.invoke(MobileCommand.FeedImportOpml, params.toJson())

    suspend fun exportBackup(params: ExportBackupParams): String = core.invoke(MobileCommand.BackupExport, params.toJson())

    suspend fun importBackup(params: ImportBackupParams): String = core.invoke(MobileCommand.BackupImport, params.toJson())

    suspend fun listFolders(params: FolderListParams): String = core.invoke(MobileCommand.FolderList, params.toJson())

    suspend fun createFolder(params: FolderCreateParams): String = core.invoke(MobileCommand.FolderCreate, params.toJson())

    suspend fun deleteFolder(params: FolderDeleteParams): String = core.invoke(MobileCommand.FolderDelete, params.toJson())

    suspend fun renameFolder(params: FolderRenameParams): String = core.invoke(MobileCommand.FolderRename, params.toJson())

    suspend fun setFolderNotify(params: FolderSetNotifyParams): String = core.invoke(MobileCommand.FolderSetNotify, params.toJson())

    suspend fun suggestContacts(params: ContactSuggestParams): String = core.invoke(MobileCommand.ContactSuggest, params.toJson())

    suspend fun contactThreads(params: ContactThreadsParams): String = core.invoke(MobileCommand.ContactThreads, params.toJson())

    suspend fun listThreads(params: ThreadListParams): String = core.invoke(MobileCommand.ThreadList, params.toJson())

    suspend fun listStarredItems(params: StarredItemsParams = StarredItemsParams()): String = core.invoke(MobileCommand.StarredItems, params.toJson())

    suspend fun allocateIdentity(params: AllocateIdentityParams): String = core.invoke(MobileCommand.AllocateIdentity, params.toJson())

    suspend fun readThread(params: ThreadReadParams): String = core.invoke(MobileCommand.ThreadRead, params.toJson())

    suspend fun readAttachment(params: AttachmentReadParams): String = core.invoke(MobileCommand.AttachmentRead, params.toJson())

    suspend fun fetchChangelog(): String = core.invoke(MobileCommand.ChangelogFetch)

    suspend fun parseSearch(query: String): String = core.invoke(MobileCommand.SearchParse, jsonObject("query" to query.jsonString()))

    suspend fun buildSearch(fields: SearchFields): String = core.invoke(MobileCommand.SearchBuild, jsonObject("fields" to fields.toJson()))

    suspend fun storageUsage(): String = core.invoke(MobileCommand.StorageUsage)

    suspend fun clearStorageCache(): String = core.invoke(MobileCommand.StorageClearCache)

    suspend fun sync(params: SyncMailParams): String = core.invoke(MobileCommand.Sync, params.toJson())

    suspend fun syncRss(params: SyncRssParams): String = core.invoke(MobileCommand.RssSync, params.toJson())

    suspend fun readRssThread(params: RssThreadParams): String = core.invoke(MobileCommand.RssThread, params.toJson())

    suspend fun markRssRead(params: RssMarkReadParams): String = core.invoke(MobileCommand.RssMarkRead, params.toJson())

    suspend fun markRssStarred(params: RssMarkStarredParams): String = core.invoke(MobileCommand.RssMarkStarred, params.toJson())

    suspend fun taskLists(): String = core.invoke(MobileCommand.TaskLists)

    suspend fun createTaskList(params: TaskListCreateParams): String = core.invoke(MobileCommand.TaskListCreate, params.toJson())

    suspend fun renameTaskList(params: TaskListRenameParams): String = core.invoke(MobileCommand.TaskListRename, params.toJson())

    suspend fun deleteTaskList(params: TaskListDeleteParams): String = core.invoke(MobileCommand.TaskListDelete, params.toJson())

    suspend fun taskItems(params: TaskItemsParams): String = core.invoke(MobileCommand.TaskItems, params.toJson())

    suspend fun threadTasks(params: TaskForThreadParams): String = core.invoke(MobileCommand.TaskForThread, params.toJson())

    suspend fun createTask(params: TaskCreateParams): String = core.invoke(MobileCommand.TaskCreate, params.toJson())

    suspend fun updateTask(params: TaskUpdateParams): String = core.invoke(MobileCommand.TaskUpdate, params.toJson())

    suspend fun setTaskDone(params: TaskSetDoneParams): String = core.invoke(MobileCommand.TaskSetDone, params.toJson())

    suspend fun deleteTask(params: TaskDeleteParams): String = core.invoke(MobileCommand.TaskDelete, params.toJson())

    suspend fun reorderTasks(params: TaskReorderParams): String = core.invoke(MobileCommand.TaskReorder, params.toJson())

    suspend fun clearCompletedTasks(params: TaskClearCompletedParams): String = core.invoke(MobileCommand.TaskClearCompleted, params.toJson())

    suspend fun restoreTasks(params: TaskRestoreParams): String = core.invoke(MobileCommand.TaskRestore, params.toJson())

    suspend fun send(params: SendMailParams): String = core.invoke(MobileCommand.Send, params.toJson())

    suspend fun respondToInvite(params: InviteRespondParams): String = core.invoke(MobileCommand.InviteRespond, params.toJson())

    suspend fun calendarEvents(params: CalendarRangeParams): String = core.invoke(MobileCommand.CalendarEvents, params.toJson())

    suspend fun createCalendarEvent(params: CalendarEventParams): String = core.invoke(MobileCommand.CalendarCreate, params.toJson())

    suspend fun updateCalendarEvent(params: CalendarEventParams): String = core.invoke(MobileCommand.CalendarUpdate, params.toJson())

    suspend fun deleteCalendarEvent(eventId: String): String = core.invoke(MobileCommand.CalendarDelete, jsonObject("event_id" to eventId.jsonString()))

    /** Take a cancelled invitation (by thread message `id`) out of the calendar. */
    suspend fun removeInviteFromCalendar(message: String): String = core.invoke(MobileCommand.InviteRemoveFromCalendar, jsonObject("message" to message.jsonString()))

    /** Put an answered invitation (by thread message `id`) back in the calendar. */
    suspend fun addInviteToCalendar(
        message: String,
        threadId: String,
    ): String = core.invoke(MobileCommand.InviteAddToCalendar, jsonObject("message" to message.jsonString(), "thread_id" to threadId.jsonString()))

    /** Hand back what `calendar.delete` returned as `restore`, untouched. */
    suspend fun restoreCalendarEvent(restore: String): String = core.invoke(MobileCommand.CalendarRestore, jsonObject("restore" to restore))

    suspend fun saveDraft(params: SaveDraftParams): String = core.invoke(MobileCommand.SaveDraft, params.toJson())

    suspend fun discardDraft(params: DiscardDraftParams): String = core.invoke(MobileCommand.DiscardDraft, params.toJson())

    suspend fun archive(params: ThreadActionParams): String = core.invoke(MobileCommand.Archive, params.archiveJson())

    suspend fun delete(params: ThreadActionParams): String = core.invoke(MobileCommand.Delete, params.deleteJson())

    suspend fun move(params: MoveThreadParams): String = core.invoke(MobileCommand.Move, params.toJson())

    suspend fun copy(params: CopyThreadParams): String = core.invoke(MobileCommand.Copy, params.toJson())

    suspend fun markRead(params: MarkReadParams): String = core.invoke(MobileCommand.MarkRead, params.toJson())

    suspend fun markAllRead(params: MarkAllReadParams): String = core.invoke(MobileCommand.MarkAllRead, params.toJson())

    suspend fun emptyFolder(params: EmptyFolderParams): String = core.invoke(MobileCommand.EmptyFolder, params.toJson())

    suspend fun markStarred(params: MarkStarredParams): String = core.invoke(MobileCommand.MarkStarred, params.toJson())

    suspend fun setTags(params: SetTagsParams): String = core.invoke(MobileCommand.SetTags, params.toJson())

    suspend fun tagList(): String = core.invoke(MobileCommand.TagList, "{}")

    /** Renames [tag] everywhere, or deletes it when [to] is null. The answer rewrites [queries], in order. */
    suspend fun replaceTag(
        tag: String,
        to: String?,
        queries: List<String>,
    ): String =
        core.invoke(
            MobileCommand.ReplaceTag,
            jsonObject("tag" to tag.jsonString(), "to" to to?.jsonString(), "queries" to queries.jsonStringArray()),
        )

    suspend fun rules(): String = core.invoke(MobileCommand.Rules, "{}")

    suspend fun setRules(rules: List<Rule>): String =
        core.invoke(
            MobileCommand.SetRules,
            rules.joinToString(separator = ",", prefix = "{\"rules\":[", postfix = "]}") { it.toJson() },
        )

    suspend fun ruleMatches(id: String): String = core.invoke(MobileCommand.RuleMatches, jsonObject("id" to id.jsonString()))

    suspend fun runRule(id: String): String = core.invoke(MobileCommand.RunRule, jsonObject("id" to id.jsonString()))

    suspend fun ruleStatus(): String = core.invoke(MobileCommand.RuleStatus, "{}")

    suspend fun stopRule(id: String): String = core.invoke(MobileCommand.StopRule, jsonObject("id" to id.jsonString()))
}
