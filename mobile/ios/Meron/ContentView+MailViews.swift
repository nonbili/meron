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
                    if pendingThreadUndo != nil {
                        Button {
                            undoPendingThreadAction()
                        } label: {
                            Label(String(localized: "buttons.undo"), systemImage: "arrow.uturn.backward")
                        }
                        .buttonStyle(.bordered)
                    }
                    Text(coreAccounts.isEmpty ? String(localized: "mobile.mail.addAccountToLoadInbox") : String(localized: "mobile.mail.readTriageReply"))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            if coreAccounts.isEmpty {
                Section {
                    VStack(alignment: .leading, spacing: 10) {
                        Label(String(localized: "mobile.mail.noAccountsConfigured"), systemImage: "tray")
                            .foregroundStyle(.secondary)
                        Button {
                            openAddAccountSetup()
                        } label: {
                            Label(String(localized: "accounts.actions.addAccount"), systemImage: "person.badge.plus")
                        }
                        .buttonStyle(.borderedProminent)
                    }
                }
            } else {
                if !mailReconnectAccounts.isEmpty {
                    Section(String(localized: "mobile.ios.needsReconnect")) {
                        ForEach(mailReconnectAccounts, id: \.id) { account in
                            VStack(alignment: .leading, spacing: 8) {
                                Label(
                                    String(localized: "mobile.mail.needsReconnect")
                                        .replacingOccurrences(of: "{account}", with: accountLabel(account)),
                                    systemImage: "key"
                                )
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
                if !coreSyncErrorMessage.isEmpty {
                    Section {
                        VStack(alignment: .leading, spacing: 8) {
                            Label(String(localized: "connectivity.syncFailed"), systemImage: "exclamationmark.triangle")
                                .font(.headline)
                                .foregroundStyle(.red)
                            Text(coreSyncErrorMessage)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                            HStack {
                                Button {
                                    handleCoreSyncErrorAction()
                                } label: {
                                    Label(
                                        iosSyncErrorLooksAuthRelated(coreSyncErrorMessage)
                                            ? String(localized: "settings.account.reconnectButton")
                                            : String(localized: "mobile.mail.syncMailbox"),
                                        systemImage: iosSyncErrorLooksAuthRelated(coreSyncErrorMessage) ? "key" : "arrow.clockwise"
                                    )
                                }
                                Button(String(localized: "buttons.cancel"), role: .cancel) {
                                    clearCoreSyncError(accountId: nil)
                                }
                            }
                        }
                    }
                }

                Section(String(localized: "mobile.ios.mailbox")) {
                    Picker(String(localized: "mobile.ios.account"), selection: $selectedCoreAccountId) {
                        if showUnifiedInbox {
                            Text(navigationUnifiedInboxLabel(
                                accounts: coreAccounts,
                                unreadCounts: accountInboxUnreadCounts,
                                showUnreadBadges: showUnreadBadges
                            ))
                                .tag(iosUnifiedAccountId)
                        }
                        ForEach(visibleNavigationAccounts, id: \.id) { account in
                            Text(navigationAccountLabel(
                                account,
                                unreadCounts: accountInboxUnreadCounts,
                                showUnreadBadges: showUnreadBadges
                            ))
                                .tag(account.id)
                        }
                    }
                    .onChange(of: selectedCoreAccountId) { _, _ in
                        pendingThreadUndo = nil
                        selectedCoreFolder = "inbox"
                        coreFolders = []
                        coreThreads = []
                        selectedMailThreadIds = []
                        selectedCoreThread = nil
                        coreMessages = []
                        mailboxCursor = ""
                        mailboxAccountCursors = [:]
                    }

                    if selectedCoreAccountId != iosUnifiedAccountId, !coreFolders.isEmpty {
                        Picker(String(localized: "mobile.ios.folder"), selection: $selectedCoreFolder) {
                            ForEach(coreFolders, id: \.name) { folder in
                                Text(folder.unread > 0 ? "\(folder.name) (\(folder.unread))" : folder.name)
                                    .tag(folder.name)
                            }
                        }
                        .onChange(of: selectedCoreFolder) { _, _ in
                            pendingThreadUndo = nil
                            selectedMailThreadIds = []
                            selectedCoreThread = nil
                            coreMessages = []
                        }
                    }

                    TextField(String(localized: "threads.searchMessages"), text: $mailSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .focused($mailFocusedField, equals: .mailboxSearch)
                        .submitLabel(.search)
                        .onSubmit {
                            searchSelectedMailbox()
                            mailFocusedField = .surface
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
                    if selectedMailboxAccountId() != iosUnifiedAccountId, !selectedMailboxAccountId().isEmpty {
                        Button {
                            openSelectedMailboxAccountSettings()
                        } label: {
                            Label(String(localized: "settings.account.accountSettings"), systemImage: "gearshape")
                        }
                    }
                    if selectedAccountIsRss(selectedCoreAccountId) {
                        Button {
                            addFeedUrl = ""
                            isAddFeedPresented = true
                        } label: {
                            Label(String(localized: "feeds.actions.addFeed"), systemImage: "plus")
                        }
                        Button {
                            opmlImportAccountId = selectedCoreAccountId
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

            Section(String(localized: "mobile.ios.inbox")) {
                if coreThreads.isEmpty {
                    Text(mailSearch.isEmpty && mailFilter == .all ? String(localized: "mobile.mail.syncSelectedMailbox") : String(localized: "mobile.mail.noSearchMatches"))
                        .foregroundStyle(.secondary)
                } else {
                    if !selectedMailThreadIds.isEmpty {
                        mailSelectionControls
                    }
                    ForEach(coreThreads, id: \.id) { thread in
                        ThreadRow(
                            thread: thread,
                            account: selectedCoreAccountId == iosUnifiedAccountId ? coreAccounts.first(where: { $0.id == thread.accountId }) : nil,
                            showSenderImages: showSenderImages,
                            selected: selectedMailThreadIds.contains(thread.id),
                            selectionActive: !selectedMailThreadIds.isEmpty,
                            onArchive: {
                                if isRssThread(thread) {
                                    removeRssFeed(thread)
                                } else {
                                    archiveThread(thread)
                                }
                            },
                            onToggleStar: { markThreadStarred(thread, starred: !thread.starred) }
                        ) {
                            if selectedMailThreadIds.isEmpty {
                                readThread(thread)
                            } else {
                                toggleMailSelection(thread)
                            }
                        } onToggleSelected: {
                            toggleMailSelection(thread)
                        } onLongPress: {
                            selectMailThread(thread)
                        } actions: {
                            Button(thread.unread ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread")) {
                                markThreadRead(thread, seen: thread.unread)
                            }
                            Button(thread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star")) {
                                markThreadStarred(thread, starred: !thread.starred)
                            }
                            if isRssThread(thread) {
                                if !thread.feedUrl.isEmpty {
                                    Button(String(localized: "mobile.ios.copyFeedUrl")) {
                                        UIPasteboard.general.string = thread.feedUrl
                                        accountStatus = String(localized: "mobile.ios.copiedFeedUrl")
                                    }
                                }
                                if !rssFeedMoveTargets(for: thread).isEmpty {
                                    Button(String(localized: "threads.actions.moveTo")) {
                                        presentMoveRssFeedDialog(thread)
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
            .onChange(of: coreThreads.map(\.id)) { _, _ in
                pruneMailSelection()
            }

            conversationSection
        }
        .listStyle(.insetGrouped)
        .searchable(
            text: $mailSearch,
            placement: .navigationBarDrawer(displayMode: .always),
            prompt: Text(String(localized: "mobile.mail.searchCachedMail"))
        )
        .onSubmit(of: .search) {
            searchSelectedMailbox()
        }
        .refreshable {
            syncSelectedAccount()
        }
        .toolbar {
            mailToolbar
        }
        .focusable()
        .focused($mailFocusedField, equals: .surface)
        .onAppear {
            focusMailShortcutSurfaceIfIdle()
        }
        .onChange(of: selectedTab) { _, tab in
            if tab == .mail {
                focusMailShortcutSurfaceIfIdle()
            }
        }
        .onKeyPress(characters: CharacterSet(charactersIn: "jkuesri#3f"), phases: [.down]) { press in
            handleMailShortcut(press)
        }
        .onKeyPress(keys: [.upArrow, .downArrow], phases: [.down]) { press in
            handleMailArrowKey(press)
        }
        .onKeyPress(keys: [.delete], phases: [.down]) { press in
            handleMailDeleteKey(press)
        }
    }

    @ToolbarContentBuilder
    var mailToolbar: some ToolbarContent {
        if selectedCoreThread != nil, selectedMailThreadIds.isEmpty {
            ToolbarItem(placement: .cancellationAction) {
                Button {
                    closeSelectedConversation()
                } label: {
                    Label(String(localized: "buttons.back"), systemImage: "chevron.backward")
                }
            }
        }

        ToolbarItemGroup(placement: .topBarTrailing) {
            if !selectedMailThreadIds.isEmpty {
                Button {
                    clearMailSelection()
                } label: {
                    Label(String(localized: "mobile.actions.clearSelection"), systemImage: "xmark.circle")
                }

                Button {
                    archiveOrRemoveSelectedMailThreads()
                } label: {
                    Label(String(localized: "threads.actions.archiveThread"), systemImage: "archivebox")
                }

                Button(role: .destructive) {
                    deleteSelectedMailThreads()
                } label: {
                    Label(String(localized: "buttons.delete"), systemImage: "trash")
                }

                mailSelectionMoreMenu
            } else if let thread = selectedCoreThread {
                Button {
                    markThreadStarred(thread, starred: !thread.starred)
                } label: {
                    Label(
                        thread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star"),
                        systemImage: thread.starred ? "star.fill" : "star"
                    )
                }

                mailConversationMoreMenu(thread)
            } else {
                if !coreAccounts.isEmpty {
                    Button {
                        openNewCompose()
                    } label: {
                        Label(String(localized: "mobile.tabs.compose"), systemImage: "square.and.pencil")
                    }
                }

                Button {
                    mailFocusedField = .mailboxSearch
                } label: {
                    Label(String(localized: "threads.searchMessages"), systemImage: "magnifyingglass")
                }

                Button {
                    syncSelectedAccount()
                } label: {
                    Label(String(localized: "mobile.mail.syncMailbox"), systemImage: "arrow.clockwise")
                }

                Menu {
                    ForEach(IosFilterMode.allCases) { mode in
                        Button {
                            mailFilter = mode
                            searchSelectedMailbox()
                        } label: {
                            Label(mode.label, systemImage: mailFilter == mode ? "checkmark.circle.fill" : "circle")
                        }
                    }
                    if coreThreads.contains(where: \.unread) {
                        Divider()
                        Button {
                            markSelectedMailboxAllRead()
                        } label: {
                            Label(String(localized: "threads.actions.markAllAsRead"), systemImage: "envelope.open")
                        }
                    }
                    if selectedMailboxAccountId() != iosUnifiedAccountId, !selectedMailboxAccountId().isEmpty {
                        Divider()
                        Button {
                            openSelectedMailboxAccountSettings()
                        } label: {
                            Label(String(localized: "settings.account.accountSettings"), systemImage: "gearshape")
                        }
                    }
                    if selectedAccountIsRss(selectedCoreAccountId) {
                        Divider()
                        Button {
                            addFeedUrl = ""
                            isAddFeedPresented = true
                        } label: {
                            Label(String(localized: "feeds.actions.addFeed"), systemImage: "plus")
                        }
                        Button {
                            opmlImportAccountId = selectedCoreAccountId
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
                } label: {
                    Label(String(localized: "threads.actions.title"), systemImage: "ellipsis.circle")
                }
            }
        }
    }

    func closeSelectedConversation() {
        selectedCoreThread = nil
        coreMessages = []
        threadSearchPresented = false
        threadSearch = ""
        activeThreadSearchIndex = 0
        conversationDetailsPresented = false
        focusMailShortcutSurfaceIfIdle()
    }

    @ViewBuilder
    func mailConversationMoreMenu(_ thread: ThreadSummary) -> some View {
        Menu {
            Button {
                threadSearchPresented.toggle()
            } label: {
                Label(String(localized: "chat.searchThread"), systemImage: "magnifyingglass")
            }

            Button {
                conversationDetailsPresented.toggle()
            } label: {
                Label(String(localized: "chat.conversationDetails"), systemImage: "info.circle")
            }

            if isRssThread(thread) {
                if !thread.feedUrl.isEmpty {
                    Button {
                        UIPasteboard.general.string = thread.feedUrl
                        accountStatus = String(localized: "mobile.ios.copiedFeedUrl")
                    } label: {
                        Label(String(localized: "feeds.copyUrl"), systemImage: "link")
                    }
                }
                if !rssFeedMoveTargets(for: thread).isEmpty {
                    Button {
                        presentMoveRssFeedDialog(thread)
                    } label: {
                        Label(String(localized: "threads.actions.moveTo"), systemImage: "folder")
                    }
                }
                Button(role: .destructive) {
                    removeRssFeed(thread)
                } label: {
                    Label(String(localized: "feeds.actions.deleteFeed"), systemImage: "trash")
                }
            } else {
                Button {
                    setCurrentConversationPrefersHtml(!currentConversationPrefersHtml())
                } label: {
                    Label(
                        currentConversationPrefersHtml() ? String(localized: "chat.viewAsPlainText") : String(localized: "chat.viewAsHtml"),
                        systemImage: currentConversationPrefersHtml() ? "text.alignleft" : "chevron.left.forwardslash.chevron.right"
                    )
                }
                Button {
                    markThreadRead(thread, seen: thread.unread)
                } label: {
                    Label(threadReadToggleActionLabel(thread), systemImage: thread.unread ? "envelope.open" : "envelope.badge")
                }
                Button {
                    archiveThread(thread)
                } label: {
                    Label(String(localized: "threads.actions.archiveThread"), systemImage: "archivebox")
                }
                Button {
                    presentMoveThreadDialog(thread)
                } label: {
                    Label(String(localized: "threads.actions.moveTo"), systemImage: "folder")
                }
                Button {
                    presentCopyThreadDialog(thread)
                } label: {
                    Label(String(localized: "threads.actions.copyTo"), systemImage: "doc.on.doc")
                }
                Button(role: .destructive) {
                    deleteThread(thread)
                } label: {
                    Label(threadDeleteActionLabel(thread), systemImage: "trash")
                }
            }
        } label: {
            Label(String(localized: "chat.moreActions"), systemImage: "ellipsis.circle")
        }
    }

    func focusMailShortcutSurfaceIfIdle() {
        guard selectedTab == .mail else { return }
        if mailFocusedField == nil || mailFocusedField == .surface {
            mailFocusedField = .surface
        }
    }

    func handleMailShortcut(_ press: KeyPress) -> KeyPress.Result {
        guard selectedTab == .mail, mailFocusedField == .surface else {
            return .ignored
        }
        let key = press.characters.lowercased()

        if key == "f", press.modifiers.contains(.command) || press.modifiers.contains(.control) {
            if press.modifiers.contains(.shift) || selectedCoreThread == nil {
                mailFocusedField = .mailboxSearch
            } else {
                threadSearchPresented = true
                mailFocusedField = .threadSearch
            }
            return .handled
        }
        if key == "e", press.modifiers.contains(.command) || press.modifiers.contains(.control) {
            guard selectedCoreThread != nil, selectedCoreThread.map({ !isRssThread($0) }) ?? false else { return .ignored }
            openQuickReplyInFullEditor()
            return .handled
        }

        guard !press.modifiers.contains(.command),
              !press.modifiers.contains(.control),
              !press.modifiers.contains(.option)
        else {
            return .ignored
        }

        switch key {
        case "j":
            guard selectedMailThreadIds.isEmpty else { return .ignored }
            selectAdjacentMailThread(1)
            return .handled
        case "k":
            guard selectedMailThreadIds.isEmpty else { return .ignored }
            selectAdjacentMailThread(-1)
            return .handled
        case "e":
            if !selectedMailThreadIds.isEmpty {
                archiveOrRemoveSelectedMailThreads()
                focusMailShortcutSurfaceIfIdle()
                return .handled
            }
            guard let thread = activeMailShortcutThread(), !isRssThread(thread) else { return .ignored }
            archiveThread(thread)
            focusMailShortcutSurfaceIfIdle()
            return .handled
        case "s":
            if !selectedMailThreadIds.isEmpty {
                markSelectedMailThreadsStarred(starred: selectedThreadsStarTarget(selectedMailThreads))
                focusMailShortcutSurfaceIfIdle()
                return .handled
            }
            guard let thread = activeMailShortcutThread() else { return .ignored }
            markThreadStarred(thread, starred: !thread.starred)
            focusMailShortcutSurfaceIfIdle()
            return .handled
        case "u":
            if !selectedMailThreadIds.isEmpty {
                markSelectedMailThreadsRead(seen: false)
                focusMailShortcutSurfaceIfIdle()
                return .handled
            }
            guard let thread = activeMailShortcutThread() else { return .ignored }
            markThreadRead(thread, seen: false)
            focusMailShortcutSurfaceIfIdle()
            return .handled
        case "r":
            guard selectedMailThreadIds.isEmpty else { return .ignored }
            guard selectedCoreThread != nil, selectedCoreThread.map({ !isRssThread($0) }) ?? false else { return .ignored }
            mailFocusedField = .quickReply
            return .handled
        case "i":
            guard selectedMailThreadIds.isEmpty else { return .ignored }
            guard selectedCoreThread != nil else { return .ignored }
            conversationDetailsPresented.toggle()
            return .handled
        case "#", "3":
            if !selectedMailThreadIds.isEmpty, press.modifiers.contains(.shift) {
                deleteSelectedMailThreads()
                focusMailShortcutSurfaceIfIdle()
                return .handled
            }
            guard press.modifiers.contains(.shift),
                  let thread = activeMailShortcutThread(),
                  !isRssThread(thread)
            else {
                return .ignored
            }
            deleteThread(thread)
            focusMailShortcutSurfaceIfIdle()
            return .handled
        default:
            return .ignored
        }
    }

    func handleMailDeleteKey(_ press: KeyPress) -> KeyPress.Result {
        guard selectedTab == .mail,
              mailFocusedField == .surface,
              !press.modifiers.contains(.command),
              !press.modifiers.contains(.control),
              !press.modifiers.contains(.option)
        else {
            return .ignored
        }
        if !selectedMailThreadIds.isEmpty {
            deleteSelectedMailThreads()
            focusMailShortcutSurfaceIfIdle()
            return .handled
        }
        guard let thread = activeMailShortcutThread(), !isRssThread(thread) else {
            return .ignored
        }
        deleteThread(thread)
        focusMailShortcutSurfaceIfIdle()
        return .handled
    }

    func handleMailArrowKey(_ press: KeyPress) -> KeyPress.Result {
        guard selectedTab == .mail,
              mailFocusedField == .surface,
              selectedMailThreadIds.isEmpty,
              !press.modifiers.contains(.command),
              !press.modifiers.contains(.control),
              !press.modifiers.contains(.option)
        else {
            return .ignored
        }
        if press.key == .downArrow {
            selectAdjacentMailThread(1)
            return .handled
        }
        if press.key == .upArrow {
            selectAdjacentMailThread(-1)
            return .handled
        }
        return .ignored
    }

    func activeMailShortcutThread() -> ThreadSummary? {
        if let selectedCoreThread {
            return selectedCoreThread
        }
        return coreThreads.first
    }

    func selectAdjacentMailThread(_ delta: Int) {
        guard let next = adjacentThreadSummary(
            coreThreads,
            currentId: selectedCoreThread?.id,
            delta: delta
        ) else {
            return
        }
        readThread(next)
        mailFocusedField = .surface
    }

    func selectAdjacentThreadFromCommandPalette(_ delta: Int) {
        let threads = iosCommandPaletteAdjacentThreadSource(
            selectedTab: selectedTab,
            mailThreads: coreThreads,
            kanbanThreads: visibleKanbanThreads
        )
        guard let next = adjacentThreadSummary(
            threads,
            currentId: selectedCoreThread?.id,
            delta: delta
        ) else {
            return
        }
        if selectedTab == .kanban, let column = kanbanColumn(containing: next) {
            openKanbanThread(next, in: column)
        } else {
            selectedTab = .mail
            readThread(next)
            mailFocusedField = .surface
        }
    }

    @ViewBuilder
    var mailSelectionControls: some View {
        HStack(spacing: 12) {
            Label("\(selectedMailThreads.count)", systemImage: "checkmark.circle.fill")
                .font(.headline)
                .foregroundStyle(.tint)
            Spacer(minLength: 8)
            Button {
                archiveOrRemoveSelectedMailThreads()
            } label: {
                Label(String(localized: "threads.actions.archiveThread"), systemImage: "archivebox")
                    .labelStyle(.iconOnly)
            }
            .accessibilityLabel(String(localized: "threads.actions.archiveThread"))
            Button(role: .destructive) {
                deleteSelectedMailThreads()
            } label: {
                Label(String(localized: "buttons.delete"), systemImage: "trash")
                    .labelStyle(.iconOnly)
            }
            .accessibilityLabel(String(localized: "buttons.delete"))
            mailSelectionMoreMenu
        }
        .buttonStyle(.borderless)
    }

    @ViewBuilder
    var mailSelectionMoreMenu: some View {
        Menu {
            let markRead = selectedThreadsMarkReadTarget(selectedMailThreads)
            Button(markRead ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread")) {
                markSelectedMailThreadsRead(seen: markRead)
            }
            let star = selectedThreadsStarTarget(selectedMailThreads)
            Button(star ? String(localized: "chat.star") : String(localized: "chat.unstar")) {
                markSelectedMailThreadsStarred(starred: star)
            }
            if selectedMailThreads.count == 1, let thread = selectedMailThreads.first {
                let availability = iosSelectionMoveCopyAvailability(
                    selectedThreads: selectedMailThreads,
                    rssMoveTargetCount: rssFeedMoveTargets(for: thread).count
                )
                Divider()
                if isRssThread(thread) {
                    if availability.canMove {
                        Button(String(localized: "threads.actions.moveTo")) {
                            clearMailSelection()
                            presentMoveRssFeedDialog(thread)
                        }
                    }
                } else {
                    if availability.canMove {
                        Button(String(localized: "threads.actions.moveTo")) {
                            clearMailSelection()
                            presentMoveThreadDialog(thread)
                        }
                    }
                    if availability.canCopy {
                        Button(String(localized: "threads.actions.copyTo")) {
                            clearMailSelection()
                            presentCopyThreadDialog(thread)
                        }
                    }
                }
            }
            Button(String(localized: "mobile.actions.clearSelection"), role: .cancel) {
                clearMailSelection()
            }
        } label: {
            Label(String(localized: "chat.moreActions"), systemImage: "ellipsis.circle")
                .labelStyle(.iconOnly)
        }
        .accessibilityLabel(String(localized: "chat.moreActions"))
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

    var selectedMailThreads: [ThreadSummary] {
        visibleSelectableThreads.filter { selectedMailThreadIds.contains($0.id) }
    }

    var visibleSelectableThreads: [ThreadSummary] {
        if selectedTab == .kanban {
            return visibleKanbanThreads
        }
        return coreThreads
    }

    var visibleKanbanThreads: [ThreadSummary] {
        visibleKanbanThreadSummaries(board: activeKanbanBoard, threadsByColumn: kanbanThreadsByColumn)
    }

    func kanbanColumn(containing thread: ThreadSummary) -> IosKanbanColumnSpec? {
        activeKanbanBoard?.columns.first { column in
            (kanbanThreadsByColumn[column.id] ?? []).contains { $0.id == thread.id }
        }
    }

    func toggleMailSelection(_ thread: ThreadSummary) {
        if selectedMailThreadIds.contains(thread.id) {
            selectedMailThreadIds.remove(thread.id)
        } else {
            selectedMailThreadIds.insert(thread.id)
        }
    }

    func selectMailThread(_ thread: ThreadSummary) {
        selectedMailThreadIds.insert(thread.id)
    }

    func clearMailSelection() {
        selectedMailThreadIds = []
    }

    func pruneMailSelection() {
        let visibleIds = Set(visibleSelectableThreads.map(\.id))
        selectedMailThreadIds = selectedMailThreadIds.intersection(visibleIds)
    }

    func archiveOrRemoveSelectedMailThreads() {
        let threads = selectedMailThreads
        clearMailSelection()
        if selectedTab != .kanban {
            let partitioned = selectedThreadsPartitionedForArchiveOrRemove(threads)
            for thread in partitioned.rss {
                performRemoveRssFeed(thread)
            }
            for thread in partitioned.mail {
                archiveThread(thread)
            }
            return
        }
        for thread in threads {
            if selectedTab == .kanban, let column = kanbanColumn(containing: thread) {
                let actionThread = kanbanActionThread(thread, in: column)
                if isRssThread(actionThread) {
                    performRemoveRssFeed(actionThread)
                } else {
                    archiveOrRemoveKanbanThread(thread, in: column)
                }
            } else if isRssThread(thread) {
                performRemoveRssFeed(thread)
            } else {
                archiveThread(thread)
            }
        }
    }

    func deleteSelectedMailThreads() {
        let threads = selectedMailThreads
        if let thread = threads.first(where: selectedMailThreadDeleteRequiresConfirmation) {
            if selectedTab == .kanban, let column = kanbanColumn(containing: thread) {
                deleteKanbanThread(thread, in: column)
            } else {
                deleteThread(thread)
            }
            return
        }
        clearMailSelection()
        for thread in threads {
            if selectedTab == .kanban, let column = kanbanColumn(containing: thread) {
                deleteKanbanThread(thread, in: column)
            } else {
                deleteThread(thread)
            }
        }
    }

    func selectedMailThreadDeleteRequiresConfirmation(_ thread: ThreadSummary) -> Bool {
        if selectedTab == .kanban, let column = kanbanColumn(containing: thread) {
            return kanbanDeleteRequiresConfirmation(thread: thread, in: column)
        }
        return threadDeleteRequiresConfirmation(thread)
    }

    func markSelectedMailThreadsRead(seen: Bool) {
        let threads = selectedMailThreads.filter { $0.unread == seen }
        clearMailSelection()
        for thread in threads {
            if selectedTab == .kanban, let column = kanbanColumn(containing: thread) {
                markKanbanThreadRead(thread, in: column)
            } else {
                markThreadRead(thread, seen: seen)
            }
        }
    }

    func markSelectedMailThreadsStarred(starred: Bool) {
        let threads = selectedMailThreads.filter { $0.starred != starred }
        clearMailSelection()
        for thread in threads {
            if selectedTab == .kanban, let column = kanbanColumn(containing: thread) {
                markKanbanThreadStarred(thread, in: column)
            } else {
                markThreadStarred(thread, starred: starred)
            }
        }
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
            Section(String(localized: "mobile.ios.conversation")) {
                if let selectedCoreThread {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text(selectedCoreThread.subject.isEmpty ? selectedCoreThread.id : selectedCoreThread.subject)
                            .font(.headline)
                            .lineLimit(2)
                        Spacer()
                        Button {
                            markThreadStarred(selectedCoreThread, starred: !selectedCoreThread.starred)
                        } label: {
                            Label(
                                selectedCoreThread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star"),
                                systemImage: selectedCoreThread.starred ? "star.fill" : "star"
                            )
                            .labelStyle(.iconOnly)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel(selectedCoreThread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star"))
                        Menu {
                            Button {
                                threadSearchPresented.toggle()
                            } label: {
                                Label(String(localized: "chat.searchThread"), systemImage: "magnifyingglass")
                            }
                            Button {
                                conversationDetailsPresented.toggle()
                            } label: {
                                Label(String(localized: "chat.conversationDetails"), systemImage: "info.circle")
                            }
                            if isRssThread(selectedCoreThread) {
                                if !selectedCoreThread.feedUrl.isEmpty {
                                    Button(String(localized: "feeds.copyUrl")) {
                                        UIPasteboard.general.string = selectedCoreThread.feedUrl
                                        accountStatus = String(localized: "mobile.ios.copiedFeedUrl")
                                    }
                                }
                                if !rssFeedMoveTargets(for: selectedCoreThread).isEmpty {
                                    Button(String(localized: "threads.actions.moveTo")) {
                                        presentMoveRssFeedDialog(selectedCoreThread)
                                    }
                                }
                                Button(String(localized: "feeds.actions.deleteFeed"), role: .destructive) {
                                    removeRssFeed(selectedCoreThread)
                                }
                            } else {
                                Button {
                                    setCurrentConversationPrefersHtml(!currentConversationPrefersHtml())
                                } label: {
                                    Label(
                                        currentConversationPrefersHtml() ? String(localized: "chat.viewAsPlainText") : String(localized: "chat.viewAsHtml"),
                                        systemImage: currentConversationPrefersHtml() ? "text.alignleft" : "chevron.left.forwardslash.chevron.right"
                                    )
                                }
                                Button(threadReadToggleActionLabel(selectedCoreThread)) {
                                    markThreadRead(selectedCoreThread, seen: selectedCoreThread.unread)
                                }
                                Button(String(localized: "threads.actions.archiveThread")) {
                                    archiveThread(selectedCoreThread)
                                }
                                Button(String(localized: "threads.actions.moveTo")) {
                                    presentMoveThreadDialog(selectedCoreThread)
                                }
                                Button(String(localized: "threads.actions.copyTo")) {
                                    presentCopyThreadDialog(selectedCoreThread)
                                }
                                Button(threadDeleteActionLabel(selectedCoreThread), role: .destructive) {
                                    deleteThread(selectedCoreThread)
                                }
                            }
                        } label: {
                            Label(String(localized: "chat.moreActions"), systemImage: "ellipsis.circle")
                                .labelStyle(.iconOnly)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel(String(localized: "chat.moreActions"))
                    }
                }

                if coreMessages.isEmpty {
                    Text(String(localized: "mobile.mail.openThreadHint"))
                        .foregroundStyle(.secondary)
                } else {
                    Picker(String(localized: "mobile.ios.view"), selection: Binding(
                        get: { currentConversationPrefersHtml() },
                        set: { setCurrentConversationPrefersHtml($0) }
                    )) {
                        Text(String(localized: "chat.htmlView")).tag(true)
                        Text(String(localized: "chat.plainView")).tag(false)
                    }
                    .pickerStyle(.segmented)
                    if currentConversationPrefersHtml() && !normalizedThreadSearch.isEmpty {
                        Text(String(localized: "mobile.ios.searchUsesPlainText"))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    if conversationDetailsPresented {
                        ConversationDetailsDisclosure(
                            participants: conversationParticipants,
                            attachments: conversationDetailsAttachments,
                            onCopyEmail: { email in
                                UIPasteboard.general.string = email
                                accountStatus = String(localized: "mobile.ios.copiedEmailAddress")
                            },
                            onComposeTo: { person in
                                composeFromAccountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
                                composeFromEmail = ""
                                composeTo = conversationParticipantComposeAddress(person)
                                composeCc = ""
                                composeBcc = ""
                                composeCcBccVisible = false
                                composeSubject = ""
                                composeBody = ""
                                attachments = []
                                composeDraftId = ""
                                composeDraftSaved = false
                                composeReturnTab = iosComposeReturnTab(from: selectedTab)
                                selectedTab = .compose
                            },
                            onOpenAttachment: openMessageAttachment,
                            onSaveAttachment: saveMessageAttachment
                        )
                    }
                    if threadSearchPresented || !threadSearch.isEmpty {
                        threadSearchControls
                    }
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
                    ForEach(Array(coreMessages.enumerated()), id: \.element.id) { index, message in
                        let dateLabel = conversationDateDividerLabel(message.dateEpochSeconds)
                        let previousDateLabel = index > 0 ? conversationDateDividerLabel(coreMessages[index - 1].dateEpochSeconds) : ""
                        let activeMatch = message.id == activeThreadSearchId
                        let activeStarredJump = message.id == starredMessageScrollTarget
                        if !dateLabel.isEmpty, dateLabel != previousDateLabel {
                            ConversationDateDivider(label: dateLabel)
                        }
                        ConversationMessageRow(
                            message: message,
                            activeSearchMatch: activeMatch || activeStarredJump,
                            searchQuery: normalizedThreadSearch,
                            renderHtml: shouldRenderHtml(message) && normalizedThreadSearch.isEmpty,
                            isOutgoing: isOutgoingConversationMessage(message),
                            ownEmails: selectedConversationOwnEmails,
                            allowRemoteImages: selectedConversationAllowsRemoteImages,
                            remoteMediaRevealed: revealedRemoteMediaMessageIds.contains(message.id),
                            isDraftContext: selectedCoreThread.map { MailStateKt.folderIsDrafts(folder: $0.folder) } ?? false,
                            canComposeFromMessage: selectedCoreThread.map { !isRssThread($0) } ?? false,
                            onOpenAttachment: openMessageAttachment,
                            onSaveAttachment: saveMessageAttachment,
                            onRevealRemoteMedia: {
                                revealedRemoteMediaMessageIds.insert(message.id)
                            },
                            onCopy: { label, value in
                                UIPasteboard.general.string = value
                                accountStatus = String(format: String(localized: "mobile.ios.copiedValue"), label.lowercased())
                            },
                            onOpenReader: { messageReaderTarget = message },
                            onForward: { openMessageCompose(message, forward: true) },
                            onEditAsNew: {
                                if let selectedCoreThread,
                                   MailStateKt.folderIsDrafts(folder: selectedCoreThread.folder)
                                {
                                    openDraftCompose(message, thread: selectedCoreThread)
                                } else {
                                    openMessageCompose(message, forward: false)
                                }
                            },
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
            .onChange(of: starredMessageScrollTarget) { _, id in
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
                .focused($mailFocusedField, equals: .threadSearch)
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
                                Label {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(attachment.displayName)
                                            .lineLimit(1)
                                        Text(draftAttachmentSizeLabel(attachment))
                                            .font(.caption2)
                                            .foregroundStyle(.secondary)
                                            .lineLimit(1)
                                    }
                                } icon: {
                                    Image(systemName: "paperclip")
                                }
                            }
                            .buttonStyle(.bordered)
                            .accessibilityLabel(String(localized: "composer.actions.removeAttachment"))
                        }
                    }
                }
            }
            TextEditor(text: $quickReplyBody)
                .frame(minHeight: 90)
                .focused($mailFocusedField, equals: .quickReply)
                .onChange(of: quickReplyBody) { _, _ in
                    quickReplyFailure = ""
                    persistSelectedQuickReplyDraft()
                }
                .onKeyPress(keys: [.return]) { press in
                    if sendShortcutMatches(press, mode: sendShortcutMode),
                       quickReplyCanSend(body: quickReplyBody, attachmentCount: quickReplyAttachments.count, sending: quickReplySending)
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
                    .disabled(!quickReplyCanSend(body: quickReplyBody, attachmentCount: quickReplyAttachments.count, sending: quickReplySending))
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
                Label(
                    quickReplySending ? String(localized: "composer.status.sending") : String(localized: "buttons.send"),
                    systemImage: quickReplySending ? "arrow.triangle.2.circlepath" : "arrowshape.turn.up.left"
                )
            }
            .disabled(!quickReplyCanSend(body: quickReplyBody, attachmentCount: quickReplyAttachments.count, sending: quickReplySending))
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

    var conversationDetailsAttachments: [MessageAttachment] {
        visibleConversationAttachments(
            messages: coreMessages,
            allowRemoteImages: selectedConversationAllowsRemoteImages,
            revealedRemoteMediaMessageIds: revealedRemoteMediaMessageIds
        )
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

    var selectedConversationOwnEmails: Set<String> {
        guard let selectedCoreThread,
              let account = coreAccounts.first(where: { $0.id == selectedCoreThread.accountId })
        else {
            return []
        }
        return Set(
            MailStateKt.accountSendIdentities(account: account)
                .map { $0.email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }
                .filter { !$0.isEmpty }
        )
    }

    func isOutgoingConversationMessage(_ message: MessageBody) -> Bool {
        let from = (message.fromAddr.isEmpty ? message.from : message.fromAddr)
            .trimmingCharacters(in: CharacterSet(charactersIn: " <>;,"))
            .lowercased()
        return !from.isEmpty && selectedConversationOwnEmails.contains(from)
    }

    var selectedConversationAllowsRemoteImages: Bool {
        guard let selectedCoreThread,
              let account = coreAccounts.first(where: { $0.id == selectedCoreThread.accountId })
        else {
            return false
        }
        return account.loadRemoteImages
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
