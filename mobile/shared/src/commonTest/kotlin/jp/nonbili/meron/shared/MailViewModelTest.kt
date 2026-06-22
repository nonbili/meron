package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MailViewModelTest {
    @Test
    fun draftRequiresRecipientSubjectAndBodyBeforeSend() {
        assertFalse(ComposeDraft(to = "a@example.com", subject = "Hi").canSend)
        assertTrue(ComposeDraft(to = "a@example.com", subject = "Hi", body = "Body").canSend)
    }

    @Test
    fun parseMailtoUrlBuildsComposeDraft() {
        val draft = parseMailtoUrl(
            "mailto:a@example.com,b@example.com?to=c@example.com&cc=d@example.com&e=ignored&subject=Hello%20there&body=Line+one%0ALine+two",
        )

        require(draft != null)
        assertEquals("a@example.com, b@example.com, c@example.com", draft.to)
        assertEquals("d@example.com", draft.cc)
        assertEquals("Hello there", draft.subject)
        assertEquals("Line one\nLine two", draft.body)
    }

    @Test
    fun parseMailtoUrlRejectsOtherSchemesAndDedupesRecipients() {
        assertEquals(null, parseMailtoUrl("https://example.com"))

        val draft = parseMailtoUrl("mailto:A@example.com?to=a@example.com;B@example.com")

        require(draft != null)
        assertEquals("A@example.com, B@example.com", draft.to)
    }

    @Test
    fun loadingThreadsClearsSyncingAndError() {
        val vm = MailViewModel()
        vm.setSyncing(true)
        vm.fail("network")
        vm.showThreads(
            listOf(
                ThreadSummary(
                    id = "t1",
                    accountId = "user1",
                    folder = "INBOX",
                    subject = "Hello",
                    sender = "user1@mail.localhost",
                ),
            ),
        )

        assertFalse(vm.state.syncing)
        assertEquals(null, vm.state.error)
        assertEquals("Hello", vm.state.threads.single().subject)
    }

    @Test
    fun selectingAccountFolderAndThreadUpdatesNavigationState() {
        val vm = MailViewModel()
        vm.showAccounts(listOf(AccountSummary(id = "a1", email = "a@example.com")))
        vm.showFolders(listOf(FolderSummary(accountId = "a1", name = "INBOX")))
        vm.showThreads(listOf(thread(id = "t1")))

        vm.selectAccount("a1")
        vm.selectFolder("INBOX")
        vm.selectThread(
            "t1",
            listOf(MessageBody(id = "m1", from = "a@example.com", to = "b@example.com", subject = "Hello", body = "Body")),
        )

        assertEquals("a1", vm.state.selectedAccountId)
        assertEquals("INBOX", vm.state.selectedFolder)
        assertEquals("t1", vm.state.selectedThreadId)
        assertEquals("m1", vm.state.selectedThread.single().id)
    }

    @Test
    fun threadActionsUpdateSharedState() {
        val vm = MailViewModel()
        vm.showThreads(listOf(thread(id = "t1", unread = true), thread(id = "t2")))

        vm.setThreadStarred("t1", true)
        vm.setThreadRead("t1", read = true)
        vm.archiveThread("t2")

        assertTrue(vm.state.threads.single { it.id == "t1" }.starred)
        assertFalse(vm.state.threads.single { it.id == "t1" }.unread)
        assertEquals(listOf("t1"), vm.state.threads.map { it.id })
    }

    @Test
    fun draftAttachmentsCanBeAddedAndRemoved() {
        val vm = MailViewModel()
        vm.updateDraft(ComposeDraft(to = "a@example.com", subject = "Hi", body = "Body"))

        vm.addDraftAttachment(
            DraftAttachment(
                id = "att1",
                displayName = "photo.jpg",
                mimeType = "image/jpeg",
                sizeBytes = 12,
                dataBase64 = "aW1hZ2U=",
            ),
        )
        vm.removeDraftAttachment("att1")

        assertEquals(emptyList(), vm.state.draft.attachments)
        assertTrue(vm.state.draft.canSend)
    }

    @Test
    fun composeDraftMapsAttachmentsToSendMailParams() {
        val params = ComposeDraft(
            to = "a@example.com",
            cc = "c@example.com",
            bcc = "b@example.com",
            subject = "Hi",
            body = "Body",
            attachments = listOf(
                DraftAttachment(
                    id = "att1",
                    displayName = "note.txt",
                    mimeType = "text/plain",
                    sizeBytes = 4,
                    dataBase64 = "Tm90ZQ==",
                ),
            ),
        ).toSendMailParams(accountId = "me@example.com", from = "sender@example.com")

        assertEquals("me@example.com", params.accountId)
        assertEquals("sender@example.com", params.from)
        assertEquals("c@example.com", params.cc)
        assertEquals("b@example.com", params.bcc)
        assertEquals("note.txt", params.attachments.single().filename)
        assertEquals("text/plain", params.attachments.single().mime)
        assertEquals("Tm90ZQ==", params.attachments.single().data)
    }

    private fun thread(id: String, unread: Boolean = false): ThreadSummary {
        return ThreadSummary(
            id = id,
            accountId = "a1",
            folder = "INBOX",
            subject = "Hello",
            sender = "a@example.com",
            unread = unread,
        )
    }
}
