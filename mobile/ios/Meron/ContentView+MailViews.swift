import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    var mailView: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text(coreAccounts.isEmpty ? "Connect your mail" : accountStatus)
                        .font(.headline)
                    Text(coreAccounts.isEmpty ? "Add an account from the Accounts tab to load your inbox." : "Read, triage, and reply from the selected mailbox.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            if coreAccounts.isEmpty {
                Section {
                    Label("No accounts configured", systemImage: "tray")
                        .foregroundStyle(.secondary)
                }
            } else {
                if !mailReconnectAccounts.isEmpty {
                    Section("Needs Reconnect") {
                        ForEach(mailReconnectAccounts, id: \.id) { account in
                            VStack(alignment: .leading, spacing: 8) {
                                Label("\(accountLabel(account)) needs credentials", systemImage: "key")
                                    .font(.headline)
                                Text("Update the password or OAuth sign-in before syncing this account.")
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                Button {
                                    reconnectAccount(account)
                                } label: {
                                    Label("Reconnect", systemImage: "arrow.triangle.2.circlepath")
                                }
                            }
                        }
                    }
                }

                Section("Mailbox") {
                    Picker("Account", selection: $selectedCoreAccountId) {
                        if showUnifiedInbox {
                            Text("Unified inbox")
                                .tag(iosUnifiedAccountId)
                        }
                        ForEach(visibleNavigationAccounts, id: \.id) { account in
                            Text(account.displayName.isEmpty ? (account.email.isEmpty ? account.id : account.email) : account.displayName)
                                .tag(account.id)
                        }
                    }
                    .onChange(of: selectedCoreAccountId) { _, _ in
                        selectedCoreFolder = "inbox"
                        coreFolders = []
                        coreThreads = []
                        selectedCoreThread = nil
                        coreMessages = []
                        mailboxCursor = ""
                        mailboxAccountCursors = [:]
                    }

                    if selectedCoreAccountId != iosUnifiedAccountId, !coreFolders.isEmpty {
                        Picker("Folder", selection: $selectedCoreFolder) {
                            ForEach(coreFolders, id: \.name) { folder in
                                Text(folder.unread > 0 ? "\(folder.name) (\(folder.unread))" : folder.name)
                                    .tag(folder.name)
                            }
                        }
                        .onChange(of: selectedCoreFolder) { _, _ in
                            selectedCoreThread = nil
                            coreMessages = []
                        }
                    }

                    TextField("Search mail", text: $mailSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.search)
                        .onSubmit {
                            searchSelectedMailbox()
                        }

                    Picker("Filter", selection: $mailFilter) {
                        ForEach(IosFilterMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: mailFilter) { _, _ in
                        searchSelectedMailbox()
                    }

                    Button {
                        syncSelectedAccount()
                    } label: {
                        Label("Sync Mailbox", systemImage: "arrow.clockwise")
                    }
                    Button {
                        searchSelectedMailbox()
                    } label: {
                        Label("Search Cached Mail", systemImage: "magnifyingglass")
                    }
                    if selectedAccountIsRss(selectedCoreAccountId) {
                        Button {
                            addFeedUrl = ""
                            isAddFeedPresented = true
                        } label: {
                            Label("Add Feed", systemImage: "plus")
                        }
                        Button {
                            isOpmlImporterPresented = true
                        } label: {
                            Label("Import OPML", systemImage: "square.and.arrow.down")
                        }
                        Button {
                            exportSelectedAccountOpml()
                        } label: {
                            Label("Export OPML", systemImage: "square.and.arrow.up")
                        }
                    }
                    if coreThreads.contains(where: \.unread) {
                        Button {
                            markSelectedMailboxAllRead()
                        } label: {
                            Label("Mark All Read", systemImage: "envelope.open")
                        }
                    }
                }
            }

            Section("Inbox") {
                if coreThreads.isEmpty {
                    Text(mailSearch.isEmpty && mailFilter == .all ? "Sync the selected mailbox to load cached threads." : "No messages match the current search and filter.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(coreThreads, id: \.id) { thread in
                        ThreadRow(thread: thread, showSenderImages: showSenderImages) {
                            readThread(thread)
                        } actions: {
                            Button(thread.unread ? "Mark Read" : "Mark Unread") {
                                markThreadRead(thread, seen: thread.unread)
                            }
                            Button(thread.starred ? "Unstar" : "Star") {
                                markThreadStarred(thread, starred: !thread.starred)
                            }
                            if isRssThread(thread) {
                                if !thread.feedUrl.isEmpty {
                                    Button("Copy Feed URL") {
                                        UIPasteboard.general.string = thread.feedUrl
                                        accountStatus = "Copied feed URL."
                                    }
                                }
                                Button("Remove Feed", role: .destructive) {
                                    removeRssFeed(thread)
                                }
                            } else {
                                Button("Archive") {
                                    archiveThread(thread)
                                }
                                Button("Move To") {
                                    presentMoveThreadDialog(thread)
                                }
                                Button("Copy To") {
                                    presentCopyThreadDialog(thread)
                                }
                                Button(threadDeleteActionLabel(thread), role: .destructive) {
                                    deleteThread(thread)
                                }
                            }
                        }
                    }
                    if !mailboxCursor.isEmpty || !mailboxAccountCursors.isEmpty || isLoadingMoreThreads {
                        Button {
                            loadMoreCoreThreads()
                        } label: {
                            if isLoadingMoreThreads {
                                Label("Loading Older", systemImage: "hourglass")
                            } else {
                                Label("Load Older", systemImage: "chevron.down")
                            }
                        }
                        .disabled(isLoadingMoreThreads)
                    }
                }
            }

            conversationSection
        }
        .listStyle(.insetGrouped)
    }

    var hiddenNavigationAccountIds: Set<String> {
        Set(hiddenNavigationAccountsValue
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty })
    }

    var visibleNavigationAccounts: [AccountSummary] {
        let hidden = hiddenNavigationAccountIds
        return coreAccounts.filter { !hidden.contains($0.id) }
    }

    var mailReconnectAccounts: [AccountSummary] {
        let selectedId = selectedMailboxAccountId()
        let accounts = coreAccounts.filter { $0.needsReconnect && !MailStateKt.accountSummaryIsRss(account: $0) }
        if selectedId == iosUnifiedAccountId || selectedId.isEmpty {
            return accounts
        }
        return accounts.filter { $0.id == selectedId }
    }

    var conversationSection: some View {
        ScrollViewReader { proxy in
            Section("Conversation") {
                if let selectedCoreThread {
                    Text(selectedCoreThread.subject.isEmpty ? selectedCoreThread.id : selectedCoreThread.subject)
                        .font(.headline)
                    if !isRssThread(selectedCoreThread) {
                        Button {
                            presentMoveThreadDialog(selectedCoreThread)
                        } label: {
                            Label("Move to Folder", systemImage: "folder")
                        }
                        Button {
                            presentCopyThreadDialog(selectedCoreThread)
                        } label: {
                            Label("Copy to Folder", systemImage: "doc.on.doc")
                        }
                    }
                }

                if coreMessages.isEmpty {
                    Text("Open a thread to read cached messages.")
                        .foregroundStyle(.secondary)
                } else {
                    Picker("View", selection: Binding(
                        get: { currentConversationPrefersHtml() },
                        set: { setCurrentConversationPrefersHtml($0) }
                    )) {
                        Text("HTML").tag(true)
                        Text("Plain Text").tag(false)
                    }
                    .pickerStyle(.segmented)
                    if currentConversationPrefersHtml() && !normalizedThreadSearch.isEmpty {
                        Text("Search uses plain text.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    ConversationDetailsDisclosure(
                        participants: conversationParticipants,
                        attachments: conversationAttachments,
                        onCopyEmail: { email in
                            UIPasteboard.general.string = email
                            accountStatus = "Copied email address."
                        },
                        onComposeTo: { person in
                            composeFromAccountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
                            composeFromEmail = ""
                            composeTo = person.email
                            composeCc = ""
                            composeBcc = ""
                            composeSubject = ""
                            composeBody = ""
                            attachments = []
                            composeDraftId = ""
                            composeDraftSaved = false
                            selectedTab = .compose
                        },
                        onOpenAttachment: openMessageAttachment
                    )
                    threadSearchControls
                    if !messageCursor.isEmpty || isLoadingMoreMessages {
                        Button {
                            loadMoreThreadMessages()
                        } label: {
                            if isLoadingMoreMessages {
                                Label("Loading Older Messages", systemImage: "hourglass")
                            } else {
                                Label("Load Older Messages", systemImage: "chevron.up")
                            }
                        }
                        .disabled(isLoadingMoreMessages)
                    }
                    ForEach(coreMessages, id: \.id) { message in
                        let activeMatch = message.id == activeThreadSearchId
                        ConversationMessageRow(
                            message: message,
                            activeSearchMatch: activeMatch,
                            renderHtml: shouldRenderHtml(message) && normalizedThreadSearch.isEmpty,
                            canComposeFromMessage: selectedCoreThread.map { !isRssThread($0) } ?? false,
                            onOpenAttachment: openMessageAttachment,
                            onCopy: { label, value in
                                UIPasteboard.general.string = value
                                accountStatus = "Copied \(label.lowercased())."
                            },
                            onForward: { openMessageCompose(message, forward: true) },
                            onEditAsNew: { openMessageCompose(message, forward: false) },
                            onToggleRead: { toggleMessageRead(message) },
                            onToggleStarred: { toggleMessageStarred(message) },
                            onDelete: { deleteMessage(message) }
                        )
                        .id(message.id)
                    }

                    if let selectedCoreThread, !isRssThread(selectedCoreThread) {
                        quickReplySection
                    }
                }
            }
            .onChange(of: activeThreadSearchId) { _, id in
                guard !id.isEmpty else { return }
                withAnimation {
                    proxy.scrollTo(id, anchor: .center)
                }
            }
        }
    }

    var threadSearchControls: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Search conversation", text: $threadSearch)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .onChange(of: threadSearch) { _, _ in
                    activeThreadSearchIndex = 0
                }
            HStack {
                Text(threadSearchMatchLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Previous") {
                    goToThreadSearchMatch(-1)
                }
                .disabled(threadSearchMatches.isEmpty)
                Button("Next") {
                    goToThreadSearchMatch(1)
                }
                .disabled(threadSearchMatches.isEmpty)
                if !threadSearch.isEmpty {
                    Button {
                        threadSearch = ""
                        activeThreadSearchIndex = 0
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.borderless)
                }
            }
        }
    }

    var quickReplySection: some View {
        Group {
            if !quickReplyAttachments.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack {
                        ForEach(quickReplyAttachments, id: \.id) { attachment in
                            Button {
                                quickReplyAttachments.removeAll { $0.id == attachment.id }
                                quickReplyFailure = ""
                            } label: {
                                Label(attachment.displayName, systemImage: "xmark.circle")
                                    .lineLimit(1)
                            }
                            .buttonStyle(.bordered)
                        }
                    }
                }
            }
            TextEditor(text: $quickReplyBody)
                .frame(minHeight: 90)
                .onChange(of: quickReplyBody) { _, _ in
                    quickReplyFailure = ""
                }
                .onKeyPress(keys: [.return]) { press in
                    if sendShortcutMatches(press, mode: sendShortcutMode),
                       !quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !quickReplyAttachments.isEmpty
                    {
                        sendQuickReply()
                        return .handled
                    }
                    return .ignored
                }
            if !quickReplyFailure.isEmpty {
                HStack {
                    Label(quickReplyFailure, systemImage: "exclamationmark.circle")
                        .font(.caption)
                        .foregroundStyle(.red)
                    Spacer()
                    Button("Retry") {
                        sendQuickReply()
                    }
                    .disabled(quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && quickReplyAttachments.isEmpty)
                }
            }
            Button {
                isQuickReplyFileImporterPresented = true
            } label: {
                Label("Attach File", systemImage: "paperclip")
            }
            Button {
                openQuickReplyInFullEditor()
            } label: {
                Label("Open Full Editor", systemImage: "arrow.up.left.and.arrow.down.right")
            }
            Button {
                sendQuickReply()
            } label: {
                Label("Send Reply", systemImage: "arrowshape.turn.up.left")
            }
            .disabled(quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && quickReplyAttachments.isEmpty)
        }
    }

    var normalizedThreadSearch: String {
        threadSearch.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }

    var threadSearchMatches: [String] {
        guard !normalizedThreadSearch.isEmpty else { return [] }
        return coreMessages
            .filter { conversationMessageSearchText($0).contains(normalizedThreadSearch) }
            .map(\.id)
    }

    var activeThreadSearchId: String {
        guard !threadSearchMatches.isEmpty else { return "" }
        return threadSearchMatches[min(activeThreadSearchIndex, threadSearchMatches.count - 1)]
    }

    var threadSearchMatchLabel: String {
        guard !normalizedThreadSearch.isEmpty else { return "" }
        return "\(threadSearchMatches.isEmpty ? 0 : min(activeThreadSearchIndex + 1, threadSearchMatches.count))/\(threadSearchMatches.count)"
    }

    func goToThreadSearchMatch(_ delta: Int) {
        let matches = threadSearchMatches
        guard !matches.isEmpty else { return }
        let next = activeThreadSearchIndex + delta
        if next < 0 {
            activeThreadSearchIndex = matches.count - 1
        } else if next >= matches.count {
            activeThreadSearchIndex = 0
        } else {
            activeThreadSearchIndex = next
        }
    }

    func conversationMessageSearchText(_ message: MessageBody) -> String {
        [
            message.subject,
            message.from,
            message.fromAddr,
            message.to,
            message.cc,
            message.body,
            message.bodyHtml.replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression),
            message.attachments.map(\.filename).joined(separator: " "),
        ].joined(separator: " ").lowercased()
    }

    var conversationAttachments: [MessageAttachment] {
        Array(coreMessages.flatMap(\.attachments).reversed())
    }

    var conversationParticipants: [ConversationParticipant] {
        guard selectedCoreThread.map({ !isRssThread($0) }) ?? false else { return [] }
        let ownEmail = selectedCoreThread
            .flatMap { thread in coreAccounts.first(where: { $0.id == thread.accountId })?.email }
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() } ?? ""
        var order: [String] = []
        var rows: [String: (name: String, email: String, count: Int, isSelf: Bool)] = [:]

        func add(name: String, email: String) {
            let normalized = email
                .trimmingCharacters(in: CharacterSet(charactersIn: " <>;,"))
                .lowercased()
            guard !normalized.isEmpty, normalized.contains("@") else { return }
            if var existing = rows[normalized] {
                existing.count += 1
                if existing.name.isEmpty || existing.name == existing.email,
                   !name.isEmpty,
                   name != email
                {
                    existing.name = name
                }
                rows[normalized] = existing
            } else {
                order.append(normalized)
                rows[normalized] = (
                    name: (!name.isEmpty && name != email) ? name : normalized,
                    email: normalized,
                    count: 1,
                    isSelf: normalized == ownEmail
                )
            }
        }

        for message in coreMessages {
            add(name: message.from, email: message.fromAddr.isEmpty ? message.from : message.fromAddr)
            parseAddressList(message.to).forEach { add(name: $0.name, email: $0.email) }
            parseAddressList(message.cc).forEach { add(name: $0.name, email: $0.email) }
        }

        return order.compactMap { rows[$0] }
            .map { ConversationParticipant(name: $0.name, email: $0.email, count: $0.count, isSelf: $0.isSelf) }
            .sorted {
                if $0.isSelf != $1.isSelf { return !$0.isSelf }
                return $0.count > $1.count
            }
    }

    func parseAddressList(_ value: String) -> [(name: String, email: String)] {
        value
            .split(whereSeparator: { $0 == "," || $0 == ";" })
            .compactMap { raw -> (name: String, email: String)? in
                let entry = raw.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !entry.isEmpty else { return nil }
                if let open = entry.lastIndex(of: "<"), let close = entry.lastIndex(of: ">"), open < close {
                    let name = entry[..<open]
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                    let email = entry[entry.index(after: open) ..< close].trimmingCharacters(in: .whitespacesAndNewlines)
                    return (name, email)
                }
                return (entry, entry)
            }
    }
}
