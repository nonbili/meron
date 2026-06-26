@testable import Meron
import MeronUI
import XCTest

final class MailtoUrlContractTests: XCTestCase {
    func testMailtoUrlPrefillsComposeFields() throws {
        let draft = try XCTUnwrap(
            MailtoKt.parseMailtoUrl(
                rawUrl: "mailto:to@example.com?cc=cc@example.com&bcc=bcc@example.com" +
                    "&subject=Mobile%20Draft&body=Line%20one%0ALine%20two"
            )
        )
        var to = ""
        var cc = ""
        var bcc = ""
        var subject = ""
        var body = ""

        applyMailtoDraftToCompose(
            draft,
            to: &to,
            cc: &cc,
            bcc: &bcc,
            subject: &subject,
            body: &body
        )

        XCTAssertEqual(to, "to@example.com")
        XCTAssertEqual(cc, "cc@example.com")
        XCTAssertEqual(bcc, "bcc@example.com")
        XCTAssertEqual(subject, "Mobile Draft")
        XCTAssertEqual(body, "Line one\nLine two")
    }

    func testComposeBodyAsSimpleHtmlEscapesAndPreservesLineBreaks() {
        let html = composeBodyAsSimpleHtml("Hello <team>\nUse \"Meron\" & mail.\n\nThanks")

        XCTAssertEqual(
            html,
            "<p>Hello &lt;team&gt;<br>Use &quot;Meron&quot; &amp; mail.</p><p>Thanks</p>"
        )
    }

    func testComposeBodyAsSimpleHtmlRendersMarkdownBlocks() {
        let html = composeBodyAsSimpleHtml(
            "## Status\n\n- **Done**\n- *Next*\n\n> See [Meron](https://example.com?q=1&ok=true)"
        )

        XCTAssertEqual(
            html,
            "<h2>Status</h2><ul><li><strong>Done</strong></li><li><em>Next</em></li></ul>" +
                #"<blockquote>See <a href="https://example.com?q=1&amp;ok=true">Meron</a></blockquote>"#
        )
    }

    func testComposeBodyAsSimpleHtmlRendersUnderlineAndStrikethrough() {
        let html = composeBodyAsSimpleHtml("__Underlined__ and ~~struck~~")

        XCTAssertEqual(html, "<p><u>Underlined</u> and <s>struck</s></p>")
    }

    func testComposeBodyAsSimpleHtmlRendersCidInlineImages() {
        let html = composeBodyAsSimpleHtml("Photo\n![diagram](cid:ios-123@meron)")

        XCTAssertEqual(html, #"<p>Photo<br><img src="cid:ios-123@meron" alt="diagram"></p>"#)
    }

    func testIosSyncErrorAuthDetectionMatchesCredentialFailures() {
        XCTAssertTrue(iosSyncErrorLooksAuthRelated("OAuth token expired"))
        XCTAssertTrue(iosSyncErrorLooksAuthRelated("535 authentication failed"))
        XCTAssertFalse(iosSyncErrorLooksAuthRelated("Network timeout"))
    }

    func testQuickReplyCanSendBlocksEmptyAndInFlightSubmissions() {
        XCTAssertFalse(quickReplyCanSend(body: "  \n", attachmentCount: 0, sending: false))
        XCTAssertTrue(quickReplyCanSend(body: "Reply", attachmentCount: 0, sending: false))
        XCTAssertTrue(quickReplyCanSend(body: "", attachmentCount: 1, sending: false))
        XCTAssertFalse(quickReplyCanSend(body: "Reply", attachmentCount: 0, sending: true))
        XCTAssertFalse(quickReplyCanSend(body: "", attachmentCount: 1, sending: true))
    }

    func testComposeReturnTabPreservesKanbanAndStarredOrigins() {
        XCTAssertEqual(iosComposeReturnTab(from: .kanban), .kanban)
        XCTAssertEqual(iosComposeReturnTab(from: .starred), .starred)
        XCTAssertEqual(iosComposeReturnTab(from: .mail), .mail)
        XCTAssertEqual(iosComposeReturnTab(from: .accounts), .mail)
        XCTAssertEqual(iosComposeReturnTab(from: .compose), .mail)
    }

    func testRssFeedMoveTargetsOnlyIncludeOtherRssAccounts() {
        let source = testAccount(id: "rss-a", email: "Feeds A", provider: "rss", authType: "rss")
        let target = testAccount(id: "rss-b", email: "Feeds B", provider: "rss", authType: "rss")
        let mail = testAccount(id: "mail", email: "mail@example.com", provider: "imap", authType: "password")

        XCTAssertEqual(
            rssFeedMoveTargetAccounts(accounts: [source, target, mail], sourceAccountId: source.id).map(\.id),
            [target.id]
        )
    }

    func testSelectionMoveCopyAvailabilityMatchesMailAndRssActions() {
        let mail = testThread(id: "acct#inbox#thread#1", unread: false, starred: false)
        let rss = testThread(id: "rss-acct#rss#feed#1", unread: false, starred: false)

        XCTAssertEqual(
            iosSelectionMoveCopyAvailability(selectedThreads: [mail], rssMoveTargetCount: 0),
            IosSelectionMoveCopyAvailability(canMove: true, canCopy: true)
        )
        XCTAssertEqual(
            iosSelectionMoveCopyAvailability(selectedThreads: [rss], rssMoveTargetCount: 1),
            IosSelectionMoveCopyAvailability(canMove: true, canCopy: false)
        )
        XCTAssertEqual(
            iosSelectionMoveCopyAvailability(selectedThreads: [rss], rssMoveTargetCount: 0),
            IosSelectionMoveCopyAvailability(canMove: false, canCopy: false)
        )
        XCTAssertEqual(
            iosSelectionMoveCopyAvailability(selectedThreads: [mail, rss], rssMoveTargetCount: 1),
            IosSelectionMoveCopyAvailability(canMove: false, canCopy: false)
        )
    }

    func testComposeDraftHasContentUsesRecipientsBodySubjectAndAttachments() {
        XCTAssertFalse(
            composeDraftHasContent(
                to: " ",
                cc: "",
                bcc: "",
                subject: "\n",
                body: "",
                attachments: []
            )
        )
        XCTAssertTrue(
            composeDraftHasContent(
                to: "",
                cc: "",
                bcc: "",
                subject: "",
                body: "Draft",
                attachments: []
            )
        )
        XCTAssertTrue(
            composeDraftHasContent(
                to: "",
                cc: "",
                bcc: "",
                subject: "",
                body: "",
                attachments: [
                    DraftAttachment(
                        id: "a1",
                        displayName: "note.txt",
                        mimeType: "text/plain",
                        sizeBytes: 4,
                        dataBase64: "ZGF0YQ=="
                    )
                ]
            )
        )
    }

    func testComposeDraftCanSendRequiresRecipientAndBodyOrAttachment() {
        let attachment = DraftAttachment(
            id: "att1",
            displayName: "file.txt",
            mimeType: "text/plain",
            sizeBytes: 4,
            dataBase64: "ZGF0YQ=="
        )

        XCTAssertFalse(composeDraftCanSend(to: "", subject: "Hi", body: "Body", attachments: []))
        XCTAssertFalse(composeDraftCanSend(to: "a@example.com", subject: "Hi", body: " ", attachments: []))
        XCTAssertTrue(composeDraftCanSend(to: "a@example.com", subject: "", body: "Body", attachments: []))
        XCTAssertTrue(composeDraftCanSend(to: "a@example.com", subject: "Hi", body: "Body", attachments: []))
        XCTAssertTrue(composeDraftCanSend(to: "a@example.com", subject: "Hi", body: " ", attachments: [attachment]))
        XCTAssertTrue(
            composeDraftCanSubmit(
                identityAvailable: true,
                to: "a@example.com",
                subject: "Hi",
                body: "Body",
                attachments: [],
                sending: false
            )
        )
        XCTAssertFalse(
            composeDraftCanSubmit(
                identityAvailable: false,
                to: "a@example.com",
                subject: "Hi",
                body: "Body",
                attachments: [],
                sending: false
            )
        )
        XCTAssertFalse(
            composeDraftCanSubmit(
                identityAvailable: true,
                to: "a@example.com",
                subject: "Hi",
                body: "Body",
                attachments: [],
                sending: true
            )
        )
        XCTAssertTrue(composeDraftNeedsNoSubjectConfirmation(subject: " "))
        XCTAssertFalse(composeDraftNeedsNoSubjectConfirmation(subject: "Hi"))
    }

    func testComposeDraftAutosaveSignatureTracksBodyAndInlineAttachments() {
        let attachment = DraftAttachment(
            id: "image-1",
            displayName: "diagram.png",
            mimeType: "image/png",
            sizeBytes: 8,
            dataBase64: "ZGF0YQ=="
        )
        let base = composeDraftAutosaveSignature(
            accountId: "acct",
            fromEmail: "me@example.com",
            to: "to@example.com",
            cc: "",
            bcc: "",
            subject: "Subject",
            body: "Body",
            rich: true,
            replyTo: "Reply <reply@example.com>",
            inReplyTo: "",
            references: "",
            inlineAttachmentIds: [],
            attachments: [attachment]
        )
        let changedBody = composeDraftAutosaveSignature(
            accountId: "acct",
            fromEmail: "me@example.com",
            to: "to@example.com",
            cc: "",
            bcc: "",
            subject: "Subject",
            body: "Edited body",
            rich: true,
            replyTo: "Reply <reply@example.com>",
            inReplyTo: "",
            references: "",
            inlineAttachmentIds: [],
            attachments: [attachment]
        )
        let inlineAttachment = composeDraftAutosaveSignature(
            accountId: "acct",
            fromEmail: "me@example.com",
            to: "to@example.com",
            cc: "",
            bcc: "",
            subject: "Subject",
            body: "Body",
            rich: true,
            replyTo: "Reply <reply@example.com>",
            inReplyTo: "",
            references: "",
            inlineAttachmentIds: ["image-1"],
            attachments: [attachment]
        )
        let changedReplyTo = composeDraftAutosaveSignature(
            accountId: "acct",
            fromEmail: "me@example.com",
            to: "to@example.com",
            cc: "",
            bcc: "",
            subject: "Subject",
            body: "Body",
            rich: true,
            replyTo: "Other <other@example.com>",
            inReplyTo: "",
            references: "",
            inlineAttachmentIds: [],
            attachments: [attachment]
        )

        XCTAssertNotEqual(base, changedBody)
        XCTAssertNotEqual(base, inlineAttachment)
        XCTAssertNotEqual(base, changedReplyTo)
    }

    func testFullReplyShortcutOnlyOpensForMailConversation() {
        let mailThread = testThread(id: "acct#inbox#thread#1", unread: false, starred: false)
        let rssThread = testThread(id: "acct#rss#feed#1", unread: false, starred: false)

        XCTAssertTrue(iosFullReplyShortcutCanOpen(selectedTab: .mail, thread: mailThread))
        XCTAssertFalse(iosFullReplyShortcutCanOpen(selectedTab: .mail, thread: rssThread))
        XCTAssertFalse(iosFullReplyShortcutCanOpen(selectedTab: .compose, thread: mailThread))
        XCTAssertFalse(iosFullReplyShortcutCanOpen(selectedTab: .mail, thread: nil))
    }

    func testThreadReadAndStarUndoTargetsInvertRequestedState() {
        XCTAssertFalse(threadReadUndoSeenTarget(afterMarkingSeen: true))
        XCTAssertTrue(threadReadUndoSeenTarget(afterMarkingSeen: false))
        XCTAssertFalse(threadStarUndoTarget(afterSettingStarred: true))
        XCTAssertTrue(threadStarUndoTarget(afterSettingStarred: false))
    }

    func testPreferredAccountAfterAccountListPrefersEmailThenNewAccountId() {
        let existing = testAccount(id: "a1", email: "old@example.com")
        let added = testAccount(id: "a2", email: "New@Example.com")
        let rss = testAccount(id: "rss-1", email: "rss-1.local", provider: "rss", authType: "rss")

        XCTAssertEqual(
            iosPreferredAccountAfterAccountList(
                accounts: [existing, added],
                preferredEmail: " new@example.com ",
                previousAccountIds: ["a1"]
            )?.id,
            "a2"
        )
        XCTAssertEqual(
            iosPreferredAccountAfterAccountList(
                accounts: [existing, rss],
                preferredEmail: nil,
                previousAccountIds: ["a1"]
            )?.id,
            "rss-1"
        )
        XCTAssertNil(
            iosPreferredAccountAfterAccountList(
                accounts: [existing],
                preferredEmail: "missing@example.com",
                previousAccountIds: ["a1"]
            )
        )
    }

    func testSelectedThreadBulkShortcutTargetsMatchSelectionMenu() {
        let unreadStarred = testThread(id: "t1", unread: true, starred: true)
        let readUnstarred = testThread(id: "t2", unread: false, starred: false)
        let readStarred = testThread(id: "t3", unread: false, starred: true)

        XCTAssertTrue(selectedThreadsMarkReadTarget([unreadStarred, readUnstarred]))
        XCTAssertFalse(selectedThreadsMarkReadTarget([readUnstarred, readStarred]))
        XCTAssertTrue(selectedThreadsStarTarget([unreadStarred, readUnstarred]))
        XCTAssertFalse(selectedThreadsStarTarget([unreadStarred, readStarred]))
    }

    func testConversationParticipantComposeAddressPreservesDisplayName() {
        XCTAssertEqual(
            conversationParticipantComposeAddress(ConversationParticipant(name: "Ada Lovelace", email: "ada@example.com", count: 1, isSelf: false)),
            "Ada Lovelace <ada@example.com>"
        )
        XCTAssertEqual(
            conversationParticipantComposeAddress(ConversationParticipant(name: "ada@example.com", email: "ada@example.com", count: 1, isSelf: false)),
            "ada@example.com"
        )
        XCTAssertEqual(
            conversationParticipantComposeAddress(ConversationParticipant(name: " ", email: "ada@example.com", count: 1, isSelf: false)),
            "ada@example.com"
        )
    }

    func testFirstRssThreadFindsFeedBeforeBulkRemove() {
        let mailThread = testThread(id: "acct#inbox#thread#1", unread: false, starred: false)
        let rssThread = testThread(id: "rss-acct#rss#feed#1", unread: false, starred: false)

        XCTAssertEqual(firstRssThread([mailThread, rssThread])?.id, "rss-acct#rss#feed#1")
        XCTAssertNil(firstRssThread([mailThread]))
    }

    func testSelectedThreadsPartitionForBulkArchiveOrRemoveKeepsMailAndRss() {
        let firstMail = testThread(id: "acct#inbox#thread#1", unread: false, starred: false)
        let rss = testThread(id: "rss-acct#rss#feed#1", unread: false, starred: false)
        let secondMail = testThread(id: "acct#inbox#thread#2", unread: false, starred: false)

        let partitioned = selectedThreadsPartitionedForArchiveOrRemove([firstMail, rss, secondMail])

        XCTAssertEqual(partitioned.mail.map(\.id), [firstMail.id, secondMail.id])
        XCTAssertEqual(partitioned.rss.map(\.id), [rss.id])
    }

    func testVisibleKanbanThreadSummariesFollowBoardOrderAndDeduplicate() {
        let inbox = IosKanbanColumnSpec(accountId: "acct", folderId: "inbox")
        let archive = IosKanbanColumnSpec(accountId: "acct", folderId: "archive")
        let board = IosKanbanBoardSpec(id: "board", name: "Board", columns: [archive, inbox])
        let duplicate = ThreadSummary(
            id: "t1",
            accountId: "acct",
            folder: "archive",
            subject: "Duplicate",
            sender: "Sender",
            preview: "",
            unread: false,
            starred: false,
            dateEpochSeconds: 0,
            feedUrl: ""
        )
        let inboxOnly = ThreadSummary(
            id: "t2",
            accountId: "acct",
            folder: "inbox",
            subject: "Inbox",
            sender: "Sender",
            preview: "",
            unread: false,
            starred: false,
            dateEpochSeconds: 0,
            feedUrl: ""
        )

        let visible = visibleKanbanThreadSummaries(
            board: board,
            threadsByColumn: [
                inbox.id: [inboxOnly, duplicate],
                archive.id: [duplicate],
            ]
        )

        XCTAssertEqual(visible.map(\.id), ["t1", "t2"])
    }

    func testAdjacentThreadSummaryClampsAndDefaultsToFirstThread() {
        let first = testThread(id: "t1", unread: false, starred: false)
        let second = testThread(id: "t2", unread: false, starred: false)

        XCTAssertEqual(adjacentThreadSummary([first, second], currentId: nil, delta: 1)?.id, first.id)
        XCTAssertEqual(adjacentThreadSummary([first, second], currentId: first.id, delta: 1)?.id, second.id)
        XCTAssertEqual(adjacentThreadSummary([first, second], currentId: second.id, delta: 1)?.id, second.id)
        XCTAssertEqual(adjacentThreadSummary([first, second], currentId: first.id, delta: -1)?.id, first.id)
        XCTAssertNil(adjacentThreadSummary([], currentId: first.id, delta: 1))
    }

    func testCommandPaletteAdjacentThreadSourceUsesKanbanOnlyOnKanbanTab() {
        let mailThread = testThread(id: "mail", unread: false, starred: false)
        let kanbanThread = testThread(id: "kanban", unread: false, starred: false)

        XCTAssertEqual(
            iosCommandPaletteAdjacentThreadSource(
                selectedTab: .mail,
                mailThreads: [mailThread],
                kanbanThreads: [kanbanThread]
            ).map(\.id),
            ["mail"]
        )
        XCTAssertEqual(
            iosCommandPaletteAdjacentThreadSource(
                selectedTab: .kanban,
                mailThreads: [mailThread],
                kanbanThreads: [kanbanThread]
            ).map(\.id),
            ["kanban"]
        )
    }

    func testKanbanStarredItemsConvertWithItemIdentityAndFilterLikeDesktopColumn() {
        let newer = StarredItemSummary(
            id: "item-new",
            threadId: "thread-1",
            accountId: "acct",
            folder: "inbox",
            subject: "Launch notes",
            sender: "Ada",
            preview: "Ready to ship",
            unread: true,
            dateEpochSeconds: 20
        )
        let older = StarredItemSummary(
            id: "item-old",
            threadId: "thread-1",
            accountId: "acct",
            folder: "archive",
            subject: "Quarterly plan",
            sender: "Bea",
            preview: "Roadmap",
            unread: false,
            dateEpochSeconds: 10
        )

        XCTAssertEqual(kanbanStarredItemsMatching([older, newer], query: "").map(\.id), ["item-new", "item-old"])
        XCTAssertEqual(kanbanStarredItemsMatching([older, newer], query: "ship").map(\.id), ["item-new"])

        let card = kanbanThreadSummary(for: newer)
        XCTAssertEqual(card.id, "item-new")
        XCTAssertEqual(card.accountId, "acct")
        XCTAssertTrue(card.starred)
        XCTAssertTrue(card.unread)
    }

    func testKanbanStarredItemActionTargetUsesMappedThreadIdentity() {
        let mailCard = ThreadSummary(
            id: "message-1",
            accountId: "acct",
            folder: "inbox",
            subject: "Mail",
            sender: "Ada",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 0,
            feedUrl: ""
        )
        let rssCard = ThreadSummary(
            id: "rss-item-1",
            accountId: "rss-acct",
            folder: "rss",
            subject: "Feed",
            sender: "Feed",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 0,
            feedUrl: ""
        )

        XCTAssertEqual(
            kanbanStarredItemActionTarget(for: mailCard, threadIdsByItemId: ["message-1": "mail-thread-1"]),
            KanbanStarredItemActionTarget(itemId: "message-1", threadId: "mail-thread-1", isRss: false)
        )
        XCTAssertEqual(
            kanbanStarredItemActionTarget(for: rssCard, threadIdsByItemId: ["rss-item-1": "rss-acct#rss#feed-1"]),
            KanbanStarredItemActionTarget(itemId: "rss-item-1", threadId: "rss-acct#rss#feed-1", isRss: true)
        )
    }

    func testKanbanDeleteConfirmationUsesThreadOrStarredMessageRules() {
        let inboxColumn = IosKanbanColumnSpec(accountId: "acct", folderId: "inbox")
        let starredColumn = IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosStarredFolderId)
        let inboxThread = testThread(id: "thread-1", unread: false, starred: false)

        XCTAssertFalse(kanbanDeleteRequiresConfirmation(thread: inboxThread, in: inboxColumn))
        XCTAssertTrue(kanbanDeleteRequiresConfirmation(thread: inboxThread, in: starredColumn))
    }

    func testIosKanbanMoveValidationRejectsMailAndRssCrossMoves() {
        let mailAccount = testAccount(id: "mail", email: "mail@example.com")
        let rssAccount = testAccount(id: "rss", email: "rss.local", provider: "rss", authType: "rss")

        XCTAssertEqual(iosKanbanMoveValidation(threadIsRss: false, targetAccount: mailAccount), .allowed)
        XCTAssertEqual(iosKanbanMoveValidation(threadIsRss: true, targetAccount: rssAccount), .allowed)
        XCTAssertEqual(iosKanbanMoveValidation(threadIsRss: true, targetAccount: mailAccount), .rssFeedToMailAccount)
        XCTAssertEqual(iosKanbanMoveValidation(threadIsRss: false, targetAccount: rssAccount), .mailThreadToRssAccount)
        XCTAssertEqual(iosKanbanMoveValidation(threadIsRss: false, targetAccount: nil), .missingTargetAccount)
    }

    func testStarredItemReaderMessagePreservesItemIdentityForRssReader() {
        let item = StarredItemSummary(
            id: "rss-item-1",
            threadId: "rss-acct#rss#feed-1",
            accountId: "rss-acct",
            folder: "rss",
            subject: "Feed item",
            sender: "Example Feed",
            preview: "Item preview",
            unread: true,
            dateEpochSeconds: 42
        )

        let message = starredItemReaderMessage(for: item)

        XCTAssertEqual(message.id, "rss-item-1")
        XCTAssertEqual(message.subject, "Feed item")
        XCTAssertEqual(message.from, "Example Feed")
        XCTAssertEqual(message.body, "Item preview")
        XCTAssertEqual(message.dateEpochSeconds, 42)
        XCTAssertTrue(message.unread)
        XCTAssertTrue(message.starred)
        XCTAssertFalse(message.hasAttachments)
    }

    func testStarredKanbanReaderMessagePreservesCardIdentityForRssReader() {
        let card = ThreadSummary(
            id: "rss-item-2",
            accountId: "rss-acct",
            folder: "rss",
            subject: "Kanban feed item",
            sender: "Example Feed",
            preview: "Kanban preview",
            unread: false,
            starred: true,
            dateEpochSeconds: 64,
            feedUrl: ""
        )

        let message = starredKanbanReaderMessage(for: card)

        XCTAssertEqual(message.id, "rss-item-2")
        XCTAssertEqual(message.subject, "Kanban feed item")
        XCTAssertEqual(message.from, "Example Feed")
        XCTAssertEqual(message.body, "Kanban preview")
        XCTAssertEqual(message.dateEpochSeconds, 64)
        XCTAssertFalse(message.unread)
        XCTAssertTrue(message.starred)
        XCTAssertFalse(message.hasAttachments)
    }

    func testStarredItemsAfterActionUpdatesRowsLikeAndroidStarredList() {
        let first = StarredItemSummary(
            id: "item-1",
            threadId: "thread-1",
            accountId: "acct",
            folder: "inbox",
            subject: "First",
            sender: "Ada",
            preview: "",
            unread: true,
            dateEpochSeconds: 10
        )
        let second = StarredItemSummary(
            id: "item-2",
            threadId: "thread-2",
            accountId: "acct",
            folder: "inbox",
            subject: "Second",
            sender: "Bea",
            preview: "",
            unread: false,
            dateEpochSeconds: 8
        )

        let markedRead = starredItemsAfterAction([first, second], itemId: "item-1", update: .read(seen: true))
        XCTAssertEqual(markedRead.map(\.id), ["item-1", "item-2"])
        XCTAssertFalse(markedRead[0].unread)
        XCTAssertFalse(markedRead[1].unread)

        let markedUnread = starredItemsAfterAction(markedRead, itemId: "item-2", update: .read(seen: false))
        XCTAssertTrue(markedUnread[1].unread)

        let removed = starredItemsAfterAction(markedUnread, itemId: "item-1", update: .remove)
        XCTAssertEqual(removed.map(\.id), ["item-2"])
    }

    func testStarredItemsAfterMessageStarredInsertsUpdatesAndRemovesRows() {
        let existing = StarredItemSummary(
            id: "old",
            threadId: "thread-old",
            accountId: "acct",
            folder: "inbox",
            subject: "Old",
            sender: "Bea",
            preview: "",
            unread: false,
            dateEpochSeconds: -1
        )
        let thread = testThread(id: "thread-1", unread: false, starred: false)
        let message = testMessageBody(id: "message-1", attachments: [])

        let inserted = starredItemsAfterMessageStarred([existing], message: message, thread: thread, starred: true)

        XCTAssertEqual(inserted.map(\.id), ["message-1", "old"])
        XCTAssertEqual(inserted[0].threadId, "thread-1")
        XCTAssertEqual(inserted[0].accountId, "acct")
        XCTAssertEqual(inserted[0].sender, "Sender")
        XCTAssertEqual(starredItemsAfterMessageStarred(inserted, message: message, thread: thread, starred: false).map(\.id), ["old"])
    }

    func testStarredItemsAfterThreadActionsUpdateEveryRowForThread() {
        let first = StarredItemSummary(
            id: "message-1",
            threadId: "thread-1",
            accountId: "acct",
            folder: "inbox",
            subject: "First",
            sender: "Ada",
            preview: "",
            unread: true,
            dateEpochSeconds: 10
        )
        let second = StarredItemSummary(
            id: "message-2",
            threadId: "thread-1",
            accountId: "acct",
            folder: "inbox",
            subject: "Second",
            sender: "Bea",
            preview: "",
            unread: true,
            dateEpochSeconds: 8
        )
        let other = StarredItemSummary(
            id: "other",
            threadId: "thread-2",
            accountId: "acct",
            folder: "inbox",
            subject: "Other",
            sender: "Cy",
            preview: "",
            unread: true,
            dateEpochSeconds: 6
        )

        let markedRead = starredItemsAfterThreadReadState([first, second, other], threadId: "thread-1", seen: true)

        XCTAssertEqual(markedRead.map(\.unread), [false, false, true])
        XCTAssertEqual(starredItemsAfterThreadStarred(markedRead, thread: testThread(id: "thread-1", unread: false, starred: true), messages: [], starred: false).map(\.id), ["other"])
    }

    func testStarredItemsAfterThreadStarredInsertsLoadedMessages() {
        let existing = StarredItemSummary(
            id: "old",
            threadId: "thread-old",
            accountId: "acct",
            folder: "inbox",
            subject: "Old",
            sender: "Bea",
            preview: "",
            unread: false,
            dateEpochSeconds: -1
        )
        let thread = testThread(id: "thread-1", unread: true, starred: false)
        let message = testMessageBody(id: "message-1", attachments: [])

        let inserted = starredItemsAfterThreadStarred([existing], thread: thread, messages: [message], starred: true)

        XCTAssertEqual(inserted.map(\.id), ["message-1", "old"])
        XCTAssertEqual(inserted[0].threadId, "thread-1")
        XCTAssertEqual(inserted[0].subject, "Hello")
        XCTAssertEqual(inserted[0].preview, "Body")
    }

    func testStarredKanbanThreadsAfterActionKeepsUnifiedStarredColumnCoherent() {
        let first = ThreadSummary(
            id: "item-1",
            accountId: "acct",
            folder: "inbox",
            subject: "First",
            sender: "Ada",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 10,
            feedUrl: ""
        )
        let second = ThreadSummary(
            id: "item-2",
            accountId: "acct",
            folder: "inbox",
            subject: "Second",
            sender: "Bea",
            preview: "",
            unread: false,
            starred: true,
            dateEpochSeconds: 8,
            feedUrl: ""
        )

        let markedRead = starredKanbanThreadsAfterAction([first, second], itemId: "item-1", update: .read(seen: true))
        XCTAssertEqual(markedRead.map(\.id), ["item-1", "item-2"])
        XCTAssertFalse(markedRead[0].unread)
        XCTAssertTrue(markedRead[0].starred)

        let removed = starredKanbanThreadsAfterAction(markedRead, itemId: "item-1", update: .remove)
        XCTAssertEqual(removed.map(\.id), ["item-2"])
    }

    func testStarredKanbanThreadActionsUseItemToThreadMapping() {
        let first = ThreadSummary(
            id: "item-1",
            accountId: "acct",
            folder: "inbox",
            subject: "First",
            sender: "Ada",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 10,
            feedUrl: ""
        )
        let second = ThreadSummary(
            id: "item-2",
            accountId: "acct",
            folder: "inbox",
            subject: "Second",
            sender: "Bea",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 8,
            feedUrl: ""
        )
        let other = ThreadSummary(
            id: "other",
            accountId: "acct",
            folder: "inbox",
            subject: "Other",
            sender: "Cy",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 6,
            feedUrl: ""
        )
        let mapping = ["item-1": "thread-1", "item-2": "thread-1", "other": "thread-2"]

        let markedRead = starredKanbanThreadsAfterThreadReadState([first, second, other], threadId: "thread-1", threadIdsByItemId: mapping, seen: true)
        XCTAssertEqual(markedRead.map(\.unread), [false, false, true])

        let unstarred = starredKanbanThreadsAfterThreadStarred(markedRead, threadId: "thread-1", threadIdsByItemId: mapping, starred: false)
        XCTAssertEqual(unstarred.map(\.id), ["other"])
    }

    func testKanbanThreadsFollowConversationMessageFlagAndDeleteUpdates() {
        let first = ThreadSummary(
            id: "thread-1",
            accountId: "acct",
            folder: "inbox",
            subject: "First",
            sender: "Ada",
            preview: "",
            unread: true,
            starred: false,
            dateEpochSeconds: 10,
            feedUrl: ""
        )
        let second = ThreadSummary(
            id: "thread-2",
            accountId: "acct",
            folder: "inbox",
            subject: "Second",
            sender: "Bea",
            preview: "",
            unread: false,
            starred: true,
            dateEpochSeconds: 8,
            feedUrl: ""
        )

        let updated = kanbanThreadsAfterThreadFlagUpdate(
            [first, second],
            threadId: "thread-1",
            unread: false,
            starred: true
        )
        XCTAssertEqual(updated.map(\.id), ["thread-1", "thread-2"])
        XCTAssertFalse(updated[0].unread)
        XCTAssertTrue(updated[0].starred)
        XCTAssertFalse(updated[1].unread)
        XCTAssertTrue(updated[1].starred)

        let removed = kanbanThreadsAfterRemovingThread(updated, threadId: "thread-1")
        XCTAssertEqual(removed.map(\.id), ["thread-2"])
    }

    func testSelectedThreadIdsAfterRemovingThreadPrunesOnlyTarget() {
        let selected = selectedThreadIdsAfterRemovingThread(
            ["thread-1", "thread-2", "thread-3"],
            threadId: "thread-2"
        )

        XCTAssertEqual(selected, ["thread-1", "thread-3"])
    }

    func testThreadsAfterMarkingThreadIdsReadOnlyUpdatesTargetedThreads() {
        let first = ThreadSummary(
            id: "thread-1",
            accountId: "acct",
            folder: "inbox",
            subject: "First",
            sender: "Ada",
            preview: "",
            unread: true,
            starred: false,
            dateEpochSeconds: 10,
            feedUrl: ""
        )
        let second = ThreadSummary(
            id: "thread-2",
            accountId: "acct",
            folder: "inbox",
            subject: "Second",
            sender: "Bea",
            preview: "",
            unread: true,
            starred: true,
            dateEpochSeconds: 8,
            feedUrl: ""
        )

        let updated = threadsAfterMarkingThreadIdsRead([first, second], threadIds: ["thread-1"])

        XCTAssertEqual(updated.map(\.id), ["thread-1", "thread-2"])
        XCTAssertFalse(updated[0].unread)
        XCTAssertTrue(updated[1].unread)
        XCTAssertTrue(updated[1].starred)
    }

    func testMessagesFollowThreadLevelReadAndStarActions() {
        let first = testMessageBody(id: "m1", attachments: [])
        let second = testMessageBody(id: "m2", attachments: [])

        let unread = messagesAfterThreadReadState([first, second], unread: true)
        XCTAssertTrue(unread.allSatisfy(\.unread))

        let read = messagesAfterThreadReadState(unread, unread: false)
        XCTAssertFalse(read.contains(where: \.unread))

        let starred = messagesAfterThreadStarredState(read, starred: true)
        XCTAssertTrue(starred.allSatisfy(\.starred))

        let unstarred = messagesAfterThreadStarredState(starred, starred: false)
        XCTAssertFalse(unstarred.contains(where: \.starred))
    }

    func testStripTrackingPixelsReplacesTinyHiddenAndTrackerImages() {
        let html = """
        <p>Hello</p>
        <img src="https://mail.example/open/abc" width="1" height="1" srcset="https://mail.example/open/2x 2x">
        <img src='https://cdn.example/logo.png' style='display: none'>
        <img src=https://analytics.example/pixel.gif>
        """

        let sanitized = stripTrackingPixelsFromHtml(html)

        XCTAssertFalse(sanitized.contains("https://mail.example/open/abc"))
        XCTAssertFalse(sanitized.contains("srcset="))
        XCTAssertFalse(sanitized.contains("width=\"1\""))
        XCTAssertFalse(sanitized.contains("height=\"1\""))
        XCTAssertFalse(sanitized.contains("https://cdn.example/logo.png"))
        XCTAssertFalse(sanitized.contains("https://analytics.example/pixel.gif"))
        XCTAssertEqual(sanitized.components(separatedBy: iosTransparentPixelDataUri).count - 1, 3)
    }

    func testStripTrackingPixelsKeepsNormalImages() {
        let html = #"<p>Hello</p><img src="https://cdn.example/photo.jpg" width="640" height="480" alt="Photo">"#

        XCTAssertEqual(stripTrackingPixelsFromHtml(html), html)
    }

    func testPreparedHtmlMessageInjectsRemoteImageCsp() {
        let blocked = preparedHtmlMessageForWebView(#"<html><head><title>Hi</title></head><body><img src="https://cdn.example/photo.jpg"></body></html>"#, allowRemoteImages: false)

        XCTAssertTrue(blocked.contains("Content-Security-Policy"))
        XCTAssertTrue(blocked.contains("img-src data: blob:"))
        XCTAssertFalse(blocked.contains("img-src * data: blob:"))

        let allowed = preparedHtmlMessageForWebView(#"<p><img src="https://cdn.example/photo.jpg"></p>"#, allowRemoteImages: true)
        XCTAssertTrue(allowed.contains("img-src * data: blob:"))
    }

    func testHtmlRemoteImageSourceCountOnlyCountsHttpImages() {
        let html = """
        <img src="https://cdn.example/photo.jpg">
        <img src='http://cdn.example/photo.png'>
        <img src="data:image/png;base64,AAAA">
        <img src="cid:logo@example.com">
        <img>
        """

        XCTAssertEqual(htmlRemoteImageSourceCount(html), 2)
    }

    func testExternalMessageNavigationUrlAllowsOnlySafeExternalSchemes() throws {
        XCTAssertEqual(externalMessageNavigationUrl(try XCTUnwrap(URL(string: "https://example.com")))?.absoluteString, "https://example.com")
        XCTAssertEqual(externalMessageNavigationUrl(try XCTUnwrap(URL(string: "http://example.com")))?.absoluteString, "http://example.com")
        XCTAssertEqual(externalMessageNavigationUrl(try XCTUnwrap(URL(string: "mailto:me@example.com")))?.absoluteString, "mailto:me@example.com")
        XCTAssertEqual(externalMessageNavigationUrl(try XCTUnwrap(URL(string: "tel:+15555550123")))?.absoluteString, "tel:+15555550123")
        XCTAssertNil(externalMessageNavigationUrl(try XCTUnwrap(URL(string: "javascript:alert(1)"))))
        XCTAssertNil(externalMessageNavigationUrl(try XCTUnwrap(URL(string: "data:text/html,hello"))))
        XCTAssertNil(externalMessageNavigationUrl(nil))
    }

    func testPlainMessageBlocksSplitFencedCodeAndNormalizeText() {
        let blocks = plainMessageBlocks(
            """
            Intro


            - item
            ```
            let value = 1
            print(value)
            ```
            Done
            """
        )

        XCTAssertEqual(blocks, [
            .text("Intro\n\n\u{2022} item"),
            .code("let value = 1\nprint(value)"),
            .text("Done"),
        ])
    }

    func testPlainMessageBlocksKeepUnclosedFenceAsText() {
        let blocks = plainMessageBlocks(
            """
            Before
            ```
            unfinished()
            """
        )

        XCTAssertEqual(blocks, [
            .text("Before"),
            .text("```\nunfinished()"),
        ])
    }

    func testPlainMessageInlinePartsParseLinksAndMarkdownLinks() {
        let parts = parsePlainMessageInlineParts(
            "Read [docs](www.example.com/deep/path/to/a/long/page) and https://meron.example/inbox"
        )

        XCTAssertEqual(parts, [
            .text("Read "),
            .link(url: "https://www.example.com/deep/path/to/a/long/page", label: "docs"),
            .text(" and "),
            .link(url: "https://meron.example/inbox", label: nil),
        ])
    }

    func testPlainMessageLinkTextShortensLikeDesktop() {
        XCTAssertEqual(normalizePlainMessageUrl("www.example.com/docs"), "https://www.example.com/docs")
        XCTAssertEqual(shortenedPlainMessageLinkText("https://www.example.com/deep/path/to/a/long/page"), "example.com/deep/path/to/a/long/pag...")
        XCTAssertEqual(shortenedPlainMessageLinkText("not a url with a very long value"), "not a url with a very long val...")
    }

    func testPlainMessageBodyLinksIgnoreCodeBlocksAndDeduplicate() {
        let links = plainMessageBodyLinks(
            """
            See [Meron](https://www.example.com/docs) and www.example.com/docs

            ```
            https://example.com/code-only
            ```

            Again https://www.example.com/docs
            """
        )

        XCTAssertEqual(links, ["https://www.example.com/docs"])
    }

    func testPlainMessageStyledRunsParseInlineMarkdownLikeDesktop() {
        XCTAssertEqual(
            parsePlainMessageStyledRuns("Use `code`, **bold**, and *italic*."),
            [
                PlainMessageStyledRun(text: "Use ", style: .normal),
                PlainMessageStyledRun(text: "code", style: .code),
                PlainMessageStyledRun(text: ", ", style: .normal),
                PlainMessageStyledRun(text: "bold", style: .bold),
                PlainMessageStyledRun(text: ", and ", style: .normal),
                PlainMessageStyledRun(text: "italic", style: .italic),
                PlainMessageStyledRun(text: ".", style: .normal),
            ]
        )
    }

    func testPlainMessageSearchRunsSplitCaseInsensitiveMatches() {
        XCTAssertEqual(
            splitPlainMessageSearchRuns("Status: Done and done again", query: "done"),
            [
                PlainMessageSearchRun(text: "Status: ", highlighted: false),
                PlainMessageSearchRun(text: "Done", highlighted: true),
                PlainMessageSearchRun(text: " and ", highlighted: false),
                PlainMessageSearchRun(text: "done", highlighted: true),
                PlainMessageSearchRun(text: " again", highlighted: false),
            ]
        )
        XCTAssertEqual(
            splitPlainMessageSearchRuns("No query", query: " "),
            [PlainMessageSearchRun(text: "No query", highlighted: false)]
        )
    }

    func testMessageMediaVisibilityHidesRemoteMediaUntilRevealedOrAllowed() {
        let keyedImage = MessageAttachment(filename: "local.png", mimeType: "image/png", sizeBytes: 1, key: "cache-1", url: "")
        let dataImage = MessageAttachment(filename: "preview.png", mimeType: "image/png", sizeBytes: 1, key: "", url: "data:image/png;base64,AAAA")
        let remoteImage = MessageAttachment(filename: "remote.png", mimeType: "image/png", sizeBytes: 1, key: "", url: "https://cdn.example/remote.png")
        let remoteVideo = MessageAttachment(filename: "remote.mp4", mimeType: "video/mp4", sizeBytes: 1, key: "", url: "https://cdn.example/remote.mp4")
        let file = MessageAttachment(filename: "note.txt", mimeType: "text/plain", sizeBytes: 1, key: "", url: "https://cdn.example/note.txt")
        let attachments = [keyedImage, dataImage, remoteImage, remoteVideo, file]

        let hidden = messageMediaVisibility(attachments: attachments, allowRemoteImages: false, revealedRemoteMedia: false)
        XCTAssertEqual(hidden.imageAttachments.map(\.filename), ["local.png", "preview.png"])
        XCTAssertTrue(hidden.videoAttachments.isEmpty)
        XCTAssertEqual(hidden.hiddenRemoteCount, 2)

        let revealed = messageMediaVisibility(attachments: attachments, allowRemoteImages: false, revealedRemoteMedia: true)
        XCTAssertEqual(revealed.imageAttachments.map(\.filename), ["local.png", "preview.png", "remote.png"])
        XCTAssertEqual(revealed.videoAttachments.map(\.filename), ["remote.mp4"])
        XCTAssertEqual(revealed.hiddenRemoteCount, 0)

        let accountAllowed = messageMediaVisibility(attachments: attachments, allowRemoteImages: true, revealedRemoteMedia: false)
        XCTAssertEqual(accountAllowed.imageAttachments.map(\.filename), ["local.png", "preview.png", "remote.png"])
        XCTAssertEqual(accountAllowed.videoAttachments.map(\.filename), ["remote.mp4"])
        XCTAssertEqual(accountAllowed.hiddenRemoteCount, 0)
    }

    func testVisibleInlineMediaKeepsVideosInHtmlMessages() {
        let visibility = MessageMediaVisibility(
            imageAttachments: [
                MessageAttachment(filename: "photo.png", mimeType: "image/png", sizeBytes: 1, key: "cache-photo", url: ""),
            ],
            videoAttachments: [
                MessageAttachment(filename: "clip.mp4", mimeType: "video/mp4", sizeBytes: 1, key: "cache-video", url: ""),
            ],
            hiddenRemoteCount: 0
        )

        let html = visibleInlineMediaAttachments(mediaVisibility: visibility, renderHtml: true)
        XCTAssertTrue(html.imageAttachments.isEmpty)
        XCTAssertEqual(html.videoAttachments.map(\.filename), ["clip.mp4"])

        let plain = visibleInlineMediaAttachments(mediaVisibility: visibility, renderHtml: false)
        XCTAssertEqual(plain.imageAttachments.map(\.filename), ["photo.png"])
        XCTAssertEqual(plain.videoAttachments.map(\.filename), ["clip.mp4"])
    }

    func testMessageReaderAttachmentGroupsIncludeVisibleMediaAndFiles() {
        let message = testMessageBody(
            id: "m1",
            attachments: [
                MessageAttachment(filename: "cached.png", mimeType: "image/png", sizeBytes: 1, key: "cache-image", url: ""),
                MessageAttachment(filename: "remote.png", mimeType: "image/png", sizeBytes: 1, key: "", url: "https://cdn.example/remote.png"),
                MessageAttachment(filename: "clip.mp4", mimeType: "video/mp4", sizeBytes: 1, key: "cache-video", url: ""),
                MessageAttachment(filename: "report.pdf", mimeType: "application/pdf", sizeBytes: 1, key: "cache-file", url: ""),
            ]
        )

        let hiddenRemote = messageReaderAttachmentGroups(message: message, allowRemoteImages: false)
        XCTAssertEqual(hiddenRemote.mediaAttachments.map(\.filename), ["cached.png", "clip.mp4"])
        XCTAssertEqual(hiddenRemote.fileAttachments.map(\.filename), ["report.pdf"])
        XCTAssertFalse(hiddenRemote.isEmpty)

        let allowedRemote = messageReaderAttachmentGroups(message: message, allowRemoteImages: true)
        XCTAssertEqual(allowedRemote.mediaAttachments.map(\.filename), ["cached.png", "remote.png", "clip.mp4"])
        XCTAssertEqual(allowedRemote.fileAttachments.map(\.filename), ["report.pdf"])
    }

    func testMessagesAfterDeletingMessageEmptiesLastMessageForConversationClose() {
        let first = testMessageBody(id: "m1", attachments: [])
        let second = testMessageBody(id: "m2", attachments: [])

        XCTAssertEqual(messagesAfterDeletingMessage([first, second], messageId: "m1").map(\.id), ["m2"])
        XCTAssertTrue(messagesAfterDeletingMessage([first], messageId: "m1").isEmpty)
    }

    func testThreadDeleteConfirmationOnlyForPermanentFolders() {
        XCTAssertFalse(threadDeleteRequiresConfirmation(folder: "Inbox"))
        XCTAssertTrue(threadDeleteRequiresConfirmation(folder: "Drafts"))
        XCTAssertTrue(threadDeleteRequiresConfirmation(folder: "Trash"))
    }

    func testFirstThreadRequiringDeleteConfirmationPreservesBulkOrder() {
        let inbox = testThread(id: "inbox", unread: false, starred: false)
        let draft = ThreadSummary(
            id: "draft",
            accountId: "acct",
            folder: "Drafts",
            subject: "Draft",
            sender: "Ada",
            preview: "",
            unread: false,
            starred: false,
            dateEpochSeconds: 0,
            feedUrl: ""
        )
        let trash = ThreadSummary(
            id: "trash",
            accountId: "acct",
            folder: "Trash",
            subject: "Trash",
            sender: "Ada",
            preview: "",
            unread: false,
            starred: false,
            dateEpochSeconds: 0,
            feedUrl: ""
        )

        XCTAssertNil(firstThreadRequiringDeleteConfirmation([inbox]))
        XCTAssertEqual(firstThreadRequiringDeleteConfirmation([inbox, draft, trash])?.id, "draft")
    }

    func testVisibleConversationAttachmentsUsesPerMessageRemoteReveal() {
        let first = testMessageBody(
            id: "m1",
            attachments: [
                MessageAttachment(filename: "local.png", mimeType: "image/png", sizeBytes: 1, key: "cache-1", url: ""),
                MessageAttachment(filename: "hidden.png", mimeType: "image/png", sizeBytes: 1, key: "", url: "https://cdn.example/hidden.png"),
                MessageAttachment(filename: "note.txt", mimeType: "text/plain", sizeBytes: 1, key: "", url: "https://cdn.example/note.txt"),
            ]
        )
        let second = testMessageBody(
            id: "m2",
            attachments: [
                MessageAttachment(filename: "revealed.mp4", mimeType: "video/mp4", sizeBytes: 1, key: "", url: "https://cdn.example/revealed.mp4"),
            ]
        )

        let visible = visibleConversationAttachments(
            messages: [first, second],
            allowRemoteImages: false,
            revealedRemoteMediaMessageIds: ["m2"]
        )

        XCTAssertEqual(visible.map(\.filename), ["revealed.mp4", "note.txt", "local.png"])
    }

    func testVisibleConversationAttachmentsIncludesAllRemoteMediaWhenAccountAllowsImages() {
        let message = testMessageBody(
            id: "m1",
            attachments: [
                MessageAttachment(filename: "remote.png", mimeType: "image/png", sizeBytes: 1, key: "", url: "https://cdn.example/remote.png"),
                MessageAttachment(filename: "remote.mp4", mimeType: "video/mp4", sizeBytes: 1, key: "", url: "https://cdn.example/remote.mp4"),
            ]
        )

        let visible = visibleConversationAttachments(
            messages: [message],
            allowRemoteImages: true,
            revealedRemoteMediaMessageIds: []
        )

        XCTAssertEqual(visible.map(\.filename), ["remote.mp4", "remote.png"])
    }

    func testImageDataUrlPayloadDecodesOnlyBase64ImageUrls() {
        XCTAssertEqual(
            imageDataUrlPayload("data:image/png;base64,SGVsbG8="),
            Data("Hello".utf8)
        )
        XCTAssertEqual(
            imageDataUrlPayload("DATA:image/jpeg;base64,AAECAw=="),
            Data([0, 1, 2, 3])
        )
        XCTAssertNil(imageDataUrlPayload("data:text/plain;base64,SGVsbG8="))
        XCTAssertNil(imageDataUrlPayload("https://cdn.example/image.png"))
    }

    func testAttachmentCanSaveRequiresCachedKey() {
        XCTAssertTrue(
            attachmentCanSave(
                MessageAttachment(filename: "cached.pdf", mimeType: "application/pdf", sizeBytes: 1, key: "acct/file.pdf", url: "")
            )
        )
        XCTAssertFalse(
            attachmentCanSave(
                MessageAttachment(filename: "remote.pdf", mimeType: "application/pdf", sizeBytes: 1, key: "", url: "https://cdn.example/file.pdf")
            )
        )
        XCTAssertFalse(
            attachmentCanSave(
                MessageAttachment(filename: "missing.pdf", mimeType: "application/pdf", sizeBytes: 1, key: "", url: "")
            )
        )
    }

    func testMessageMetadataRowsMatchIncomingVisibilityRules() {
        let message = MessageBody(
            id: "m1",
            from: "Sender",
            to: "Me <me@example.com>, Other <other@example.com>",
            cc: "Copy <copy@example.com>",
            bcc: "",
            subject: "Hello",
            body: "Body",
            bodyHtml: "",
            dateEpochSeconds: 0,
            fromAddr: "sender@example.com",
            replyTo: "Team <team@example.com>",
            messageId: "m1@example.com",
            references: "",
            unread: false,
            starred: false,
            hasAttachments: false,
            attachments: []
        )

        let rows = messageMetadataRows(message: message, isOutgoing: false, ownEmails: ["me@example.com"])

        XCTAssertEqual(rows.map(\.rawValue), [
            "Me <me@example.com>, Other <other@example.com>",
            "Team <team@example.com>",
            "Copy <copy@example.com>",
        ])
        XCTAssertTrue(messageMetadataRows(message: message, isOutgoing: true, ownEmails: ["me@example.com"]).isEmpty)
    }

    func testMessageMetadataRowsHideSoleSelfRecipientAndMatchingReplyTo() {
        let message = MessageBody(
            id: "m2",
            from: "Sender",
            to: "Me <me@example.com>",
            cc: "",
            bcc: "",
            subject: "Hello",
            body: "Body",
            bodyHtml: "",
            dateEpochSeconds: 0,
            fromAddr: "sender@example.com",
            replyTo: "Sender <sender@example.com>",
            messageId: "m2@example.com",
            references: "",
            unread: false,
            starred: false,
            hasAttachments: false,
            attachments: []
        )

        XCTAssertTrue(messageMetadataRows(message: message, isOutgoing: false, ownEmails: ["me@example.com"]).isEmpty)
    }

    func testIosCommandMatchesLabelKeywordsAndId() {
        let command = IosCommand(
            id: "mail.sync",
            label: "Sync mailbox",
            keywords: "refresh fetch check",
            systemImage: "arrow.clockwise",
            active: false,
            role: nil,
            action: {}
        )

        XCTAssertTrue(iosCommandMatches(command, query: "sync"))
        XCTAssertTrue(iosCommandMatches(command, query: "fetch"))
        XCTAssertTrue(iosCommandMatches(command, query: "mail.sync"))
        XCTAssertFalse(iosCommandMatches(command, query: "kanban"))
    }

    func testMessageReaderAddressRowsIncludeFullHeaders() {
        let message = MessageBody(
            id: "m3",
            from: "Sender",
            to: "Me <me@example.com>",
            cc: "Copy <copy@example.com>",
            bcc: "Blind <blind@example.com>",
            subject: "Hello",
            body: "Body",
            bodyHtml: "",
            dateEpochSeconds: 0,
            fromAddr: "sender@example.com",
            replyTo: "Team <team@example.com>",
            messageId: "m3@example.com",
            references: "",
            unread: false,
            starred: false,
            hasAttachments: false,
            attachments: []
        )

        let rows = messageReaderAddressRows(message: message)

        XCTAssertEqual(rows.map(\.rawValue), [
            "Sender <sender@example.com>",
            "Me <me@example.com>",
            "Copy <copy@example.com>",
            "Blind <blind@example.com>",
            "Team <team@example.com>",
        ])
    }

    private func testMessageBody(id: String, attachments: [MessageAttachment]) -> MessageBody {
        MessageBody(
            id: id,
            from: "Sender",
            to: "Me <me@example.com>",
            cc: "",
            bcc: "",
            subject: "Hello",
            body: "Body",
            bodyHtml: "",
            dateEpochSeconds: 0,
            fromAddr: "sender@example.com",
            replyTo: "",
            messageId: "\(id)@example.com",
            references: "",
            unread: false,
            starred: false,
            hasAttachments: !attachments.isEmpty,
            attachments: attachments
        )
    }

    private func testThread(id: String, unread: Bool, starred: Bool) -> ThreadSummary {
        ThreadSummary(
            id: id,
            accountId: "acct",
            folder: "inbox",
            subject: "Subject",
            sender: "Sender",
            preview: "",
            unread: unread,
            starred: starred,
            dateEpochSeconds: 0,
            feedUrl: ""
        )
    }

    private func testAccount(id: String, email: String, provider: String = "", authType: String = "") -> AccountSummary {
        AccountSummary(
            id: id,
            email: email,
            displayName: "",
            senderName: "",
            avatarUrl: "",
            needsReconnect: false,
            engine: "",
            provider: provider,
            authType: authType,
            imapHost: "",
            imapPort: 0,
            smtpHost: "",
            smtpPort: 0,
            loadRemoteImages: false,
            includedInUnified: true,
            muted: false,
            paused: false,
            conversationHtml: true,
            rssSyncIntervalMinutes: 60,
            aliases: [],
            chatWallpaperKind: "",
            chatWallpaperPresetId: "",
            chatWallpaperUrl: ""
        )
    }
}
