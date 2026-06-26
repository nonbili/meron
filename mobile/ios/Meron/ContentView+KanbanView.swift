import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    var kanbanView: some View {
        List {
            Section {
                if kanbanBoards.isEmpty {
                    Text(String(localized: "mobile.ios.loadAccountsToCreateBoard"))
                        .foregroundStyle(.secondary)
                } else {
                    Picker(String(localized: "kanban.board.label"), selection: $activeKanbanBoardId) {
                        ForEach(kanbanBoards) { board in
                            Text(board.name).tag(board.id)
                        }
                    }
                    .onChange(of: activeKanbanBoardId) { _, next in
                        selectKanbanBoard(next)
                    }
                    HStack {
                        let boardIndex = activeKanbanBoardIndex()
                        Button {
                            moveActiveKanbanBoard(delta: -1)
                        } label: {
                            Label(String(localized: "mobile.accounts.moveUp"), systemImage: "arrow.up")
                        }
                        .disabled(boardIndex <= 0)

                        Button {
                            moveActiveKanbanBoard(delta: 1)
                        } label: {
                            Label(String(localized: "mobile.accounts.moveDown"), systemImage: "arrow.down")
                        }
                        .disabled(boardIndex < 0 || boardIndex >= kanbanBoards.count - 1)
                    }

                    HStack {
                        KanbanBoardStylePreview(board: activeKanbanBoard)

                        TextField(String(localized: "kanban.board.name"), text: $kanbanNewBoardName)
                        Button {
                            renameActiveKanbanBoard()
                        } label: {
                            Label(String(localized: "mobile.ios.renameBoard"), systemImage: "pencil")
                        }
                    }

                    TextField(String(localized: "mobile.ios.boardImageUrl"), text: $kanbanBoardAvatarUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button {
                        if let board = activeKanbanBoard {
                            kanbanBoardMediaImportTarget = KanbanBoardMediaImportTarget(boardId: board.id, isWallpaper: false)
                            isKanbanBoardMediaImporterPresented = true
                        }
                    } label: {
                        Label(String(localized: "kanban.board.chooseImage"), systemImage: "photo")
                    }

                    IosWallpaperPreview(
                        presetId: kanbanBoardWallpaperPresetId,
                        customUrl: kanbanBoardWallpaperUrl
                    )
                    IosWallpaperPresetPicker(selected: Binding(
                        get: { kanbanBoardWallpaperPresetId },
                        set: { presetId in
                            kanbanBoardWallpaperPresetId = presetId
                            kanbanBoardWallpaperUrl = ""
                        }
                    ))

                    TextField(String(localized: "mobile.ios.wallpaperImageUrl"), text: $kanbanBoardWallpaperUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button {
                        if let board = activeKanbanBoard {
                            kanbanBoardMediaImportTarget = KanbanBoardMediaImportTarget(boardId: board.id, isWallpaper: true)
                            isKanbanBoardMediaImporterPresented = true
                        }
                    } label: {
                        Label(String(localized: "settings.chooseWallpaperImage"), systemImage: "photo.on.rectangle")
                    }

                    Button {
                        updateActiveKanbanBoardAppearance()
                    } label: {
                        Label(String(localized: "mobile.ios.saveBoardStyle"), systemImage: "photo")
                    }
                }

                Button {
                    createKanbanBoard()
                } label: {
                    Label(String(localized: "kanban.actions.addBoardShort"), systemImage: "plus.rectangle.on.rectangle")
                }

                Button(role: .destructive) {
                    deleteActiveKanbanBoard()
                } label: {
                    Label(String(localized: "kanban.board.delete"), systemImage: "trash")
                }
                .disabled(kanbanBoards.isEmpty)
            }

            Section(String(localized: "mobile.ios.columns")) {
                if coreAccounts.isEmpty {
                    Button {
                        openAddAccountSetup()
                    } label: {
                        Label(String(localized: "accounts.actions.addAccount"), systemImage: "person.badge.plus")
                    }
                    .buttonStyle(.borderedProminent)

                    Button {
                        listAccounts()
                    } label: {
                        Label(String(localized: "mobile.accounts.reloadAccounts"), systemImage: "person.crop.circle.badge")
                    }
                } else {
                    Button {
                        addKanbanColumn(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
                    } label: {
                        Label(String(localized: "mobile.ios.addUnifiedInbox"), systemImage: "tray.2")
                    }
                    Button {
                        addKanbanColumn(accountId: iosUnifiedAccountId, folderId: iosStarredFolderId)
                    } label: {
                        Label(String(localized: "kanban.columns.unifiedStarred"), systemImage: "star")
                    }

                    Picker(String(localized: "mobile.ios.account"), selection: $kanbanSelectedAccountId) {
                        ForEach(coreAccounts, id: \.id) { account in
                            Text(accountLabel(account)).tag(account.id)
                        }
                    }
                    .onChange(of: kanbanSelectedAccountId) { _, accountId in
                        loadFoldersForKanbanAccount(accountId)
                    }

                    Picker(String(localized: "mobile.ios.folder"), selection: $kanbanSelectedFolderId) {
                        ForEach(coreFolders, id: \.name) { folder in
                            Text(folder.name).tag(folder.name)
                        }
                    }

                    Button {
                        addKanbanColumn(accountId: kanbanSelectedAccountId, folderId: kanbanSelectedFolderId)
                    } label: {
                        Label(String(localized: "kanban.actions.addColumn"), systemImage: "rectangle.badge.plus")
                    }

                    HStack {
                        TextField(String(localized: "folders.newFolderOrLabel"), text: $kanbanCreateFolderName)
                        Button {
                            createFolderAndColumn()
                        } label: {
                            Label(String(localized: "folders.create"), systemImage: "folder.badge.plus")
                        }
                    }
                    .disabled(selectedKanbanAccountIsRss())
                }
            }

            Section(String(localized: "kanban.board.label")) {
                if let board = activeKanbanBoard {
                    TextField(String(localized: "kanban.searchBoard"), text: $kanbanSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.search)
                        .onSubmit {
                            loadActiveKanbanBoard(refresh: false)
                        }
                    Picker(String(localized: "kanban.actions.searchColumn"), selection: $kanbanSearchScope) {
                        Text(String(localized: "filters.all")).tag("all")
                        ForEach(board.columns) { column in
                            Text(kanbanColumnTitle(column)).tag(column.id)
                        }
                    }
                    .onChange(of: kanbanSearchScope) { _, _ in
                        if !kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                            loadActiveKanbanBoard(refresh: false)
                        }
                    }

                    Picker(String(localized: "filters.label"), selection: $kanbanFilter) {
                        ForEach(IosFilterMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: kanbanFilter) { _, _ in
                        loadActiveKanbanBoard(refresh: false)
                    }

                    HStack {
                        Button {
                            loadActiveKanbanBoard(refresh: false)
                        } label: {
                            Label(String(localized: "kanban.searchBoardAction"), systemImage: "magnifyingglass")
                        }
                        if !kanbanSearch.isEmpty {
                            Button {
                                kanbanSearch = ""
                                loadActiveKanbanBoard(refresh: false)
                            } label: {
                                Label(String(localized: "common.clearSearch"), systemImage: "xmark.circle")
                            }
                        }
                    }

                    if !selectedMailThreadIds.isEmpty {
                        mailSelectionControls
                    }

                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(Array(board.columns.enumerated()), id: \.element.id) { index, column in
                                let title = kanbanColumnTitle(column)
                                let threads = kanbanThreadsByColumn[column.id] ?? []
                                if kanbanMinimizedColumnIds.contains(column.id) {
                                    KanbanMinimizedColumnView(
                                        title: title,
                                        unreadCount: threads.filter(\.unread).count,
                                        onRestore: { kanbanMinimizedColumnIds.remove(column.id) }
                                    )
                                } else {
                                    KanbanColumnView(
                                        title: title,
                                        status: kanbanStatusByColumn[column.id] ?? "",
                                        threads: threads,
                                        canLoadMore: !isUnifiedStarredKanbanColumn(column) && kanbanSearchQuery(for: column).isEmpty && ((column.accountId == iosUnifiedAccountId && !(kanbanAccountCursorsByColumn[column.id] ?? [:]).isEmpty) || (column.accountId != iosUnifiedAccountId && !(kanbanCursorByColumn[column.id] ?? "").isEmpty)),
                                        isLoadingMore: kanbanLoadingMoreColumns.contains(column.id),
                                        moveTargets: { thread in kanbanMoveTargets(for: thread, from: column, in: board) },
                                        targetTitle: { kanbanColumnTitle($0) },
                                        onRefresh: { loadKanbanColumn(column, refresh: true) },
                                        onLoadMore: { loadMoreKanbanColumn(column) },
                                        onMarkAllRead: { markKanbanColumnAllRead(column) },
                                        onRemoveColumn: { removeKanbanColumn(column) },
                                        canMoveColumnLeft: index > 0,
                                        canMoveColumnRight: index < board.columns.count - 1,
                                        onMoveColumnLeft: { moveKanbanColumn(column, delta: -1) },
                                        onMoveColumnRight: { moveKanbanColumn(column, delta: 1) },
                                        onMinimizeColumn: { kanbanMinimizedColumnIds.insert(column.id) },
                                        onSearchColumn: {
                                            kanbanSearchScope = column.id
                                            if !kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                                loadActiveKanbanBoard(refresh: false)
                                            }
                                        },
                                        onOpen: { openKanbanThread($0, in: column) },
                                        onArchive: { archiveOrRemoveKanbanThread($0, in: column) },
                                        onDelete: { deleteKanbanThread($0, in: column) },
                                        onCopyFeedUrl: { thread in
                                            UIPasteboard.general.string = thread.feedUrl
                                            accountStatus = String(localized: "mobile.ios.copiedFeedUrl")
                                        },
                                        onToggleRead: { markKanbanThreadRead($0, in: column) },
                                        onToggleStar: { markKanbanThreadStarred($0, in: column) },
                                        onMove: { thread, target in moveKanbanThread(thread, from: column, to: target) },
                                        isRssFeed: { isKanbanThreadRss($0, in: column) },
                                        selectedThreadIds: selectedMailThreadIds,
                                        selectionActive: !selectedMailThreadIds.isEmpty,
                                        onToggleSelected: { toggleMailSelection($0) },
                                        onLongPress: { selectMailThread($0) },
                                        showSenderImages: showSenderImages,
                                        columnWidth: CGFloat(kanbanColumnWidth)
                                    )
                                }
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    Button {
                        loadActiveKanbanBoard(refresh: true)
                    } label: {
                        Label(String(localized: "mobile.actions.refresh"), systemImage: "arrow.clockwise")
                    }
                } else {
                    Text(String(localized: "mobile.ios.createBoardAndColumns"))
                        .foregroundStyle(.secondary)
                }
            }
        }
        .listStyle(.insetGrouped)
        .searchable(
            text: $kanbanSearch,
            placement: .navigationBarDrawer(displayMode: .always),
            prompt: Text(String(localized: "kanban.searchBoard"))
        )
        .onSubmit(of: .search) {
            loadActiveKanbanBoard(refresh: false)
        }
        .toolbar {
            kanbanToolbar
        }
        .onKeyPress(keys: [.upArrow, .downArrow], phases: [.down]) { press in
            handleKanbanArrowKey(press)
        }
        .onChange(of: visibleKanbanThreads.map(\.id)) { _, _ in
            pruneMailSelection()
        }
    }

    @ToolbarContentBuilder
    var kanbanToolbar: some ToolbarContent {
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
            } else {
                if coreAccounts.contains(where: { !MailStateKt.accountSummaryIsRss(account: $0) }) {
                    Button {
                        openNewCompose()
                    } label: {
                        Label(String(localized: "mobile.tabs.compose"), systemImage: "square.and.pencil")
                    }
                }

                Menu {
                    ForEach(IosFilterMode.allCases) { mode in
                        Button {
                            kanbanFilter = mode
                            loadActiveKanbanBoard(refresh: true)
                        } label: {
                            Label(mode.label, systemImage: kanbanFilter == mode ? "checkmark.circle.fill" : "circle")
                        }
                    }

                    Divider()
                    Button {
                        loadActiveKanbanBoard(refresh: true)
                    } label: {
                        Label(String(localized: "mobile.actions.refreshBoard"), systemImage: "arrow.clockwise")
                    }

                    Divider()
                    Button {
                        if coreAccounts.isEmpty {
                            listAccounts()
                        } else {
                            addKanbanColumn(accountId: kanbanSelectedAccountId, folderId: kanbanSelectedFolderId)
                        }
                    } label: {
                        Label(String(localized: "kanban.actions.addColumn"), systemImage: "rectangle.badge.plus")
                    }
                    .disabled(coreAccounts.isEmpty || kanbanSelectedAccountId.isEmpty || kanbanSelectedFolderId.isEmpty)

                    Button {
                        openActiveKanbanBoardSettings()
                    } label: {
                        Label(String(localized: "kanban.actions.boardOptions"), systemImage: "slider.horizontal.3")
                    }
                    .disabled(activeKanbanBoard == nil)
                } label: {
                    Label(String(localized: "kanban.actions.boardOptions"), systemImage: "ellipsis.circle")
                }
            }
        }
    }

    func openActiveKanbanBoardSettings() {
        guard let board = activeKanbanBoard else { return }
        activeKanbanBoardId = board.id
        UserDefaults.standard.set(board.id, forKey: "ios_kanban_active_board_v1")
        selectedTab = .accounts
    }

    func handleKanbanArrowKey(_ press: KeyPress) -> KeyPress.Result {
        guard selectedTab == .kanban,
              selectedMailThreadIds.isEmpty,
              !press.modifiers.contains(.command),
              !press.modifiers.contains(.control),
              !press.modifiers.contains(.option)
        else {
            return .ignored
        }
        if press.key == .downArrow {
            selectAdjacentKanbanThread(1)
            return .handled
        }
        if press.key == .upArrow {
            selectAdjacentKanbanThread(-1)
            return .handled
        }
        return .ignored
    }

    func selectAdjacentKanbanThread(_ delta: Int) {
        guard let next = adjacentThreadSummary(
            visibleKanbanThreads,
            currentId: selectedCoreThread?.id,
            delta: delta
        ), let column = kanbanColumn(containing: next)
        else {
            return
        }
        openKanbanThread(next, in: column)
    }
}
