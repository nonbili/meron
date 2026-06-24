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
                    Text("Load accounts to create a board.")
                        .foregroundStyle(.secondary)
                } else {
                    Picker("Board", selection: $activeKanbanBoardId) {
                        ForEach(kanbanBoards) { board in
                            Text(board.name).tag(board.id)
                        }
                    }
                    .onChange(of: activeKanbanBoardId) { _, next in
                        UserDefaults.standard.set(next, forKey: "ios_kanban_active_board_v1")
                        kanbanNewBoardName = activeKanbanBoard?.name ?? ""
                        kanbanBoardAvatarUrl = activeKanbanBoard?.avatarUrl ?? ""
                        kanbanBoardWallpaperPresetId = activeKanbanBoard?.wallpaperPresetId ?? ""
                        kanbanBoardWallpaperUrl = activeKanbanBoard?.wallpaperUrl ?? ""
                        loadActiveKanbanBoard(refresh: false)
                    }

                    HStack {
                        KanbanBoardStylePreview(board: activeKanbanBoard)

                        TextField("Board name", text: $kanbanNewBoardName)
                        Button {
                            renameActiveKanbanBoard()
                        } label: {
                            Label("Rename", systemImage: "pencil")
                        }
                    }

                    TextField("Board image URL", text: $kanbanBoardAvatarUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    TextField("Wallpaper preset", text: $kanbanBoardWallpaperPresetId)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    TextField("Wallpaper image URL", text: $kanbanBoardWallpaperUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    Button {
                        updateActiveKanbanBoardAppearance()
                    } label: {
                        Label("Save Board Style", systemImage: "photo")
                    }
                }

                Button {
                    createKanbanBoard()
                } label: {
                    Label("New Board", systemImage: "plus.rectangle.on.rectangle")
                }

                Button(role: .destructive) {
                    deleteActiveKanbanBoard()
                } label: {
                    Label("Delete Board", systemImage: "trash")
                }
                .disabled(kanbanBoards.isEmpty)
            }

            Section("Columns") {
                if coreAccounts.isEmpty {
                    Button {
                        listAccounts()
                    } label: {
                        Label("Load Accounts", systemImage: "person.crop.circle.badge")
                    }
                } else {
                    Button {
                        addKanbanColumn(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
                    } label: {
                        Label("Add Unified Inbox", systemImage: "tray.2")
                    }

                    Picker("Account", selection: $kanbanSelectedAccountId) {
                        ForEach(coreAccounts, id: \.id) { account in
                            Text(accountLabel(account)).tag(account.id)
                        }
                    }
                    .onChange(of: kanbanSelectedAccountId) { _, accountId in
                        loadFoldersForKanbanAccount(accountId)
                    }

                    Picker("Folder", selection: $kanbanSelectedFolderId) {
                        ForEach(coreFolders, id: \.name) { folder in
                            Text(folder.name).tag(folder.name)
                        }
                    }

                    Button {
                        addKanbanColumn(accountId: kanbanSelectedAccountId, folderId: kanbanSelectedFolderId)
                    } label: {
                        Label("Add Column", systemImage: "rectangle.badge.plus")
                    }

                    HStack {
                        TextField("New folder", text: $kanbanCreateFolderName)
                        Button {
                            createFolderAndColumn()
                        } label: {
                            Label("Create", systemImage: "folder.badge.plus")
                        }
                    }
                    .disabled(selectedKanbanAccountIsRss())
                }
            }

            Section("Board") {
                if let board = activeKanbanBoard {
                    TextField("Search board", text: $kanbanSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.search)
                        .onSubmit {
                            loadActiveKanbanBoard(refresh: false)
                        }

                    Picker("Filter", selection: $kanbanFilter) {
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
                            Label("Search Board", systemImage: "magnifyingglass")
                        }
                        if !kanbanSearch.isEmpty {
                            Button {
                                kanbanSearch = ""
                                loadActiveKanbanBoard(refresh: false)
                            } label: {
                                Label("Clear", systemImage: "xmark.circle")
                            }
                        }
                    }

                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(board.columns) { column in
                                KanbanColumnView(
                                    title: kanbanColumnTitle(column),
                                    status: kanbanStatusByColumn[column.id] ?? "",
                                    threads: kanbanThreadsByColumn[column.id] ?? [],
                                    canLoadMore: kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && ((column.accountId == iosUnifiedAccountId && !(kanbanAccountCursorsByColumn[column.id] ?? [:]).isEmpty) || (column.accountId != iosUnifiedAccountId && !(kanbanCursorByColumn[column.id] ?? "").isEmpty)),
                                    isLoadingMore: kanbanLoadingMoreColumns.contains(column.id),
                                    moveTargets: board.columns.filter { $0.id != column.id && $0.accountId != iosUnifiedAccountId },
                                    targetTitle: { kanbanColumnTitle($0) },
                                    onRefresh: { loadKanbanColumn(column, refresh: true) },
                                    onLoadMore: { loadMoreKanbanColumn(column) },
                                    onMarkAllRead: { markKanbanColumnAllRead(column) },
                                    onRemoveColumn: { removeKanbanColumn(column) },
                                    onOpen: { readThread($0) },
                                    onArchive: { archiveOrRemoveKanbanThread($0, in: column) },
                                    onDelete: { deleteKanbanThread($0, in: column) },
                                    onToggleRead: { markKanbanThreadRead($0, in: column) },
                                    onToggleStar: { markKanbanThreadStarred($0, in: column) },
                                    onMove: { thread, target in moveKanbanThread(thread, from: column, to: target) },
                                    showSenderImages: showSenderImages,
                                    columnWidth: CGFloat(kanbanColumnWidth)
                                )
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    Button {
                        loadActiveKanbanBoard(refresh: true)
                    } label: {
                        Label("Refresh Board", systemImage: "arrow.clockwise")
                    }
                } else {
                    Text("Create a board and add folder columns.")
                        .foregroundStyle(.secondary)
                }
            }
        }
        .listStyle(.insetGrouped)
    }

}
