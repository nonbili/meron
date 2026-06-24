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
                    Text(coreAccounts.isEmpty ? String(localized: "mobile.mail.connectYourMail") : accountStatus)
                        .font(.headline)
                    Text(coreAccounts.isEmpty ? String(localized: "mobile.mail.addAccountToLoadInbox") : String(localized: "mobile.mail.readTriageReply"))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            if coreAccounts.isEmpty {
                Section {
                    Label(String(localized: "mobile.mail.noAccountsConfigured"), systemImage: "tray")
                        .foregroundStyle(.secondary)
                }
            } else {
                if !mailReconnectAccounts.isEmpty {
                    Section("Needs Reconnect") {
                        ForEach(mailReconnectAccounts, id: \.id) { account in
                            VStack(alignment: .leading, spacing: 8) {
                                Label("\(accountLabel(account)) needs credentials", systemImage: "key")
                                    .font(.headline)
                                Text(String(localized: "mobile.mail.reconnectHint"))
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                Button {
                                    reconnectAccount(account)
                                } label: {
                                    Label(String(localized: "settings.account.reconnectButton"), systemImage: "arrow.triangle.2.circlepath")
                                }
                            }
                        }
                    }
                }

                Section("Mailbox") {
                    Picker("Account", selection: $selectedCoreAccountId) {
                        if showUnifiedInbox {
                            Text(String(localized: "kanban.columns.unifiedInbox"))
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

                    TextField(String(localized: "threads.searchMessages"), text: $mailSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.search)
                        .onSubmit {
                            searchSelectedMailbox()
                        }

                    Picker(String(localized: "filters.label"), selection: $mailFilter) {
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
                        Label(String(localized: "mobile.mail.syncMailbox"), systemImage: "arrow.clockwise")
                    }
                    Button {
                        searchSelectedMailbox()
                    } label: {
                        Label(String(localized: "mobile.mail.searchCachedMail"), systemImage: "magnifyingglass")
                    }
                    if selectedAccountIsRss(selectedCoreAccountId) {
                        Button {
                            addFeedUrl = ""
                            isAddFeedPresented = true
                        } label: {
                            Label(String(localized: "feeds.actions.addFeed"), systemImage: "plus")
                        }
                        Button {
                            isOpmlImporterPresented = true
                        } label: {
                            Label(String(localized: "common.import"), systemImage: "square.and.arrow.down")
                        }
                        Button {
                            exportSelectedAccountOpml()
                        } label: {
                            Label(String(localized: "common.export"), systemImage: "square.and.arrow.up")
                        }
                    }
                    if coreThreads.contains(where: \.unread) {
                        Button {
                            markSelectedMailboxAllRead()
                        } label: {
                            Label(String(localized: "threads.actions.markAllAsRead"), systemImage: "envelope.open")
                        }
                    }
                }
            }

            Section("Inbox") {
                if coreThreads.isEmpty {
                    Text(mailSearch.isEmpty && mailFilter == .all ? String(localized: "mobile.mail.syncSelectedMailbox") : String(localized: "mobile.mail.noSearchMatches"))
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(coreThreads, id: \.id) { thread in
                        ThreadRow(thread: thread, showSenderImages: showSenderImages) {
                            readThread(thread)
                        } actions: {
                            Button(thread.unread ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread")) {
                                markThreadRead(thread, seen: thread.unread)
                            }
                            Button(thread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star")) {
                                markThreadStarred(thread, starred: !thread.starred)
                            }
                            if isRssThread(thread) {
                                if !thread.feedUrl.isEmpty {
                                    Button("Copy Feed URL") {
                                        UIPasteboard.general.string = thread.feedUrl
                                        accountStatus = "Copied feed URL."
                                    }
                                }
                                Button(String(localized: "feeds.actions.deleteFeed"), role: .destructive) {
                                    removeRssFeed(thread)
                                }
                            } else {
                                Button(String(localized: "threads.actions.archiveThread")) {
                                    archiveThread(thread)
                                }
                                Button(String(localized: "threads.actions.moveTo")) {
                                    presentMoveThreadDialog(thread)
                                }
                                Button(String(localized: "threads.actions.copyTo")) {
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
                                Label(String(localized: "common.loading"), systemImage: "hourglass")
                            } else {
                                Label(String(localized: "threads.actions.loadMore"), systemImage: "chevron.down")
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
                            Label(String(localized: "threads.actions.moveTo"), systemImage: "folder")
                        }
                        Button {
                            presentCopyThreadDialog(selectedCoreThread)
                        } label: {
                            Label(String(localized: "threads.actions.copyTo"), systemImage: "doc.on.doc")
                        }
                    }
                }

                if coreMessages.isEmpty {
                    Text(String(localized: "mobile.mail.openThreadHint"))
                        .foregroundStyle(.secondary)
                } else {
                    Picker("View", selection: Binding(
                        get: { currentConversationPrefersHtml() },
                        set: { setCurrentConversationPrefersHtml($0) }
                    )) {
                        Text(String(localized: "chat.htmlView")).tag(true)
                        Text(String(localized: "chat.plainView")).tag(false)
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
                                Label(String(localized: "common.loading"), systemImage: "hourglass")
                            } else {
                                Label(String(localized: "threads.actions.loadMore"), systemImage: "chevron.up")
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
            TextField(String(localized: "chat.searchThread"), text: $threadSearch)
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
                Button(String(localized: "chat.previousMatch")) {
                    goToThreadSearchMatch(-1)
                }
                .disabled(threadSearchMatches.isEmpty)
                Button(String(localized: "chat.nextMatch")) {
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
                    Button(String(localized: "chat.retry")) {
                        sendQuickReply()
                    }
                    .disabled(quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && quickReplyAttachments.isEmpty)
                }
            }
            Button {
                isQuickReplyFileImporterPresented = true
            } label: {
                Label(String(localized: "composer.actions.attachFiles"), systemImage: "paperclip")
            }
            Button {
                openQuickReplyInFullEditor()
            } label: {
                Label(String(localized: "composer.actions.openFullEditor"), systemImage: "arrow.up.left.and.arrow.down.right")
            }
            Button {
                sendQuickReply()
            } label: {
                Label(String(localized: "buttons.send"), systemImage: "arrowshape.turn.up.left")
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
