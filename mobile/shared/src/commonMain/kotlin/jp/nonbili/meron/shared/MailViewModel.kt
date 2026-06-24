package jp.nonbili.meron.shared

class MailViewModel(
    initialState: MailUiState = MailUiState(),
) {
    var state: MailUiState = initialState
        private set

    fun setSyncing(syncing: Boolean) {
        state = state.copy(syncing = syncing, error = null)
    }

    fun showAccounts(accounts: List<AccountSummary>) {
        state =
            state.copy(
                accounts = accounts,
                selectedAccountId = state.selectedAccountId ?: accounts.firstOrNull()?.id,
                error = null,
            )
    }

    fun selectAccount(accountId: String) {
        require(state.accounts.any { it.id == accountId }) { "Unknown account: $accountId" }
        state =
            state.copy(
                selectedAccountId = accountId,
                selectedFolder = null,
                selectedThreadId = null,
                selectedThread = emptyList(),
                error = null,
            )
    }

    fun showFolders(folders: List<FolderSummary>) {
        state =
            state.copy(
                folders = folders,
                selectedFolder = state.selectedFolder ?: folders.firstOrNull()?.name,
                error = null,
            )
    }

    fun selectFolder(folder: String) {
        require(state.folders.any { it.name == folder }) { "Unknown folder: $folder" }
        state =
            state.copy(
                selectedFolder = folder,
                selectedThreadId = null,
                selectedThread = emptyList(),
                error = null,
            )
    }

    fun showThreads(threads: List<ThreadSummary>) {
        state = state.copy(threads = threads, syncing = false, error = null)
    }

    fun selectThread(
        threadId: String,
        messages: List<MessageBody>,
    ) {
        require(state.threads.any { it.id == threadId }) { "Unknown thread: $threadId" }
        state = state.copy(selectedThreadId = threadId, selectedThread = messages, error = null)
    }

    fun showThread(messages: List<MessageBody>) {
        state = state.copy(selectedThread = messages, error = null)
    }

    fun updateDraft(draft: ComposeDraft) {
        state = state.copy(draft = draft)
    }

    fun addDraftAttachment(attachment: DraftAttachment) {
        state = state.copy(draft = state.draft.copy(attachments = state.draft.attachments + attachment), error = null)
    }

    fun removeDraftAttachment(attachmentId: String) {
        state =
            state.copy(
                draft = state.draft.copy(attachments = state.draft.attachments.filterNot { it.id == attachmentId }),
                error = null,
            )
    }

    fun archiveThread(threadId: String) {
        removeThread(threadId)
    }

    fun deleteThread(threadId: String) {
        removeThread(threadId)
    }

    fun setThreadStarred(
        threadId: String,
        starred: Boolean,
    ) {
        updateThread(threadId) { it.copy(starred = starred) }
    }

    fun setThreadRead(
        threadId: String,
        read: Boolean,
    ) {
        updateThread(threadId) { it.copy(unread = !read) }
    }

    fun fail(message: String) {
        state = state.copy(syncing = false, error = message)
    }

    private fun removeThread(threadId: String) {
        val existed = state.threads.any { it.id == threadId }
        require(existed) { "Unknown thread: $threadId" }
        state =
            state.copy(
                threads = state.threads.filterNot { it.id == threadId },
                selectedThreadId = state.selectedThreadId.takeUnless { it == threadId },
                selectedThread = if (state.selectedThreadId == threadId) emptyList() else state.selectedThread,
                error = null,
            )
    }

    private fun updateThread(
        threadId: String,
        update: (ThreadSummary) -> ThreadSummary,
    ) {
        val existed = state.threads.any { it.id == threadId }
        require(existed) { "Unknown thread: $threadId" }
        state = state.copy(threads = state.threads.map { if (it.id == threadId) update(it) else it }, error = null)
    }
}
