import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    var composeView: some View {
        List {
            Section {
                let identities = composeIdentityCandidates()
                if identities.count > 1 {
                    Picker(String(localized: "composer.fields.from"), selection: Binding(
                        get: { selectedComposeIdentity().map(identityKey) ?? "" },
                        set: { key in
                            let parts = key.split(separator: "|", maxSplits: 1).map(String.init)
                            if parts.count == 2 {
                                composeFromAccountId = parts[0]
                                composeFromEmail = parts[1]
                            }
                        }
                    )) {
                        ForEach(Array(identities.enumerated()), id: \.offset) { _, identity in
                            Text(MailStateKt.formatSendIdentity(identity: identity))
                                .tag(identityKey(identity))
                        }
                    }
                }
                TextField(String(localized: "composer.fields.to"), text: $composeTo)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeTo) { _, value in loadRecipientSuggestions(field: "to", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "to", value: composeTo) }
                contactSuggestionRows(field: "to")
                if shouldShowComposeCcBcc {
                    Button {
                        composeCcBccVisible = false
                    } label: {
                        Label(String(localized: "composer.actions.hideCcBcc"), systemImage: "chevron.up")
                    }
                    .buttonStyle(.borderless)
                    TextField(String(localized: "composer.fields.cc"), text: $composeCc)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: composeCc) { _, value in loadRecipientSuggestions(field: "cc", value: value) }
                        .onTapGesture { loadRecipientSuggestions(field: "cc", value: composeCc) }
                    contactSuggestionRows(field: "cc")
                    TextField(String(localized: "composer.fields.bcc"), text: $composeBcc)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: composeBcc) { _, value in loadRecipientSuggestions(field: "bcc", value: value) }
                        .onTapGesture { loadRecipientSuggestions(field: "bcc", value: composeBcc) }
                    contactSuggestionRows(field: "bcc")
                } else {
                    Button {
                        composeCcBccVisible = true
                    } label: {
                        Label(String(localized: "composer.actions.ccBcc"), systemImage: "person.2")
                    }
                    .buttonStyle(.borderless)
                }
                TextField(String(localized: "composer.fields.subject"), text: $composeSubject)
                Picker(String(localized: "composer.modes.richText"), selection: $composeRichText) {
                    Text(String(localized: "composer.modes.plainText")).tag(false)
                    Text(String(localized: "composer.modes.richText")).tag(true)
                }
                .pickerStyle(.segmented)
                .accessibilityLabel(
                    composeRichText
                        ? String(localized: "composer.actions.switchToPlainText")
                        : String(localized: "composer.actions.switchToRichText")
                )
                if composeRichText {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.bold"),
                                systemImage: "bold",
                                snippet: "**bold text**"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.italic"),
                                systemImage: "italic",
                                snippet: "*italic text*"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.underline"),
                                systemImage: "underline",
                                snippet: "__underlined text__"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.strikethrough"),
                                systemImage: "strikethrough",
                                snippet: "~~struck text~~"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.heading"),
                                systemImage: "textformat.size",
                                snippet: "## Heading"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.bulletList"),
                                systemImage: "list.bullet",
                                snippet: "- List item"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.numberedList"),
                                systemImage: "list.number",
                                snippet: "1. List item"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.quote"),
                                systemImage: "quote.bubble",
                                snippet: "> Quote"
                            )
                            composeMarkdownButton(
                                label: String(localized: "composer.toolbar.link"),
                                systemImage: "link",
                                snippet: "[link](https://example.com)"
                            )
                        }
                        .padding(.vertical, 2)
                    }
                }
                TextEditor(text: $composeBody)
                    .frame(minHeight: 220)
            }

            Section(String(localized: "mobile.ios.attachments")) {
                Button {
                    isFileImporterPresented = true
                } label: {
                    Label(String(localized: "composer.actions.attachFiles"), systemImage: "paperclip")
                }
                if composeRichText {
                    Button {
                        isInlineImageImporterPresented = true
                    } label: {
                        Label(String(localized: "composer.actions.insertInlineImage"), systemImage: "photo")
                    }
                }
                if let attachmentError {
                    Text(attachmentError)
                        .foregroundStyle(.red)
                }
                let visibleAttachments = attachments.filter { !composeInlineAttachmentIds.contains($0.id) }
                if visibleAttachments.isEmpty {
                    Text(String(localized: "mobile.ios.noAttachmentsSelected"))
                        .foregroundStyle(.secondary)
                } else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack {
                            ForEach(visibleAttachments, id: \.id) { attachment in
                                Button {
                                    attachments.removeAll { $0.id == attachment.id }
                                    composeInlineAttachmentIds.remove(attachment.id)
                                    attachmentError = nil
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
                                        Image(systemName: "xmark.circle")
                                    }
                                }
                                .buttonStyle(.bordered)
                                .accessibilityLabel(String(localized: "composer.actions.removeAttachment"))
                            }
                        }
                    }
                    Button(String(localized: "mobile.ios.clearAttachments"), role: .destructive) {
                        attachments = []
                        composeInlineAttachmentIds = []
                    }
                }
            }

            Section {
                if let draft = mailtoDraft {
                    Text(String(format: String(localized: "mobile.ios.mailtoDraftLoaded"), draft.to))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Button {
                    saveComposeDraft()
                } label: {
                    Label(String(localized: "composer.actions.saveDraft"), systemImage: "tray.and.arrow.down")
                }
                .disabled(composeSending)
                Button(role: .destructive) {
                    requestDiscardComposeDraft()
                } label: {
                    Label(String(localized: "threads.actions.discardDraft"), systemImage: "trash")
                }
                .disabled(composeSending)
                Button {
                    sendCoreMail()
                } label: {
                    Label(
                        composeSending ? String(localized: "composer.status.sending") : String(localized: "buttons.send"),
                        systemImage: composeSending ? "arrow.triangle.2.circlepath" : "paperplane.fill"
                    )
                }
                .disabled(!composeCanSend)
            }
        }
        .onKeyPress(keys: [.return]) { press in
            if sendShortcutMatches(press, mode: sendShortcutMode), composeCanSend {
                sendCoreMail()
                return .handled
            }
            return .ignored
        }
        .listStyle(.insetGrouped)
        .toolbar {
            composeToolbar
        }
        .onChange(of: currentComposeAutosaveSignature()) { _, _ in
            scheduleComposeAutosave()
        }
        .alert(String(localized: "mobile.compose.discardDraftTitle"), isPresented: $composeDiscardConfirming) {
            Button(String(localized: "buttons.discard"), role: .destructive) {
                discardComposeDraft()
            }
            Button(String(localized: "mobile.compose.keepEditing"), role: .cancel) {}
        } message: {
            Text(String(localized: "mobile.compose.discardDraftText"))
        }
    }

    @ToolbarContentBuilder
    var composeToolbar: some ToolbarContent {
        ToolbarItem(placement: .cancellationAction) {
            Button {
                closeComposeSurface()
            } label: {
                Label(String(localized: "buttons.close"), systemImage: "xmark")
            }
            .disabled(composeSending)
        }

        ToolbarItemGroup(placement: .topBarTrailing) {
            Button {
                isFileImporterPresented = true
            } label: {
                Label(String(localized: "composer.actions.attachFiles"), systemImage: "paperclip")
            }
            .disabled(composeSending)

            Button {
                sendCoreMail()
            } label: {
                Label(String(localized: "buttons.send"), systemImage: "paperplane.fill")
            }
            .disabled(!composeCanSend)

            Menu {
                Button {
                    saveComposeDraft()
                } label: {
                    Label(String(localized: "composer.actions.saveDraft"), systemImage: "tray.and.arrow.down")
                }
                .disabled(composeSending)

                Button(role: .destructive) {
                    requestDiscardComposeDraft()
                } label: {
                    Label(String(localized: "threads.actions.discardDraft"), systemImage: "trash")
                }
                .disabled(composeSending)
            } label: {
                Label(String(localized: "common.more"), systemImage: "ellipsis.circle")
            }
        }
    }

    func composeMarkdownButton(label: String, systemImage: String, snippet: String) -> some View {
        Button {
            appendComposeMarkdownSnippet(snippet)
        } label: {
            Label(label, systemImage: systemImage)
                .labelStyle(.iconOnly)
        }
        .buttonStyle(.bordered)
        .accessibilityLabel(label)
    }

    @ViewBuilder
    func contactSuggestionRows(field: String) -> some View {
        if recipientSuggestionField == field, !recipientSuggestions.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack {
                    ForEach(recipientSuggestions, id: \.addr) { contact in
                        Button {
                            acceptRecipientSuggestion(field: field, contact: contact)
                        } label: {
                            Text(MailStateKt.formatContactSuggestion(contact: contact))
                                .lineLimit(1)
                        }
                        .buttonStyle(.bordered)
                    }
                }
            }
        }
    }

    var starredView: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text(String(localized: "mobile.tabs.starred"))
                        .font(.headline)
                    Text(String(localized: "mobile.mail.starredSubtitle"))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
            }

            Section(String(localized: "mobile.ios.starredItems")) {
                let visibleItems = filteredStarredItems
                if starredItems.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Label(String(localized: "empty.noStarredItems"), systemImage: "star")
                            .font(.headline)
                        Text(String(localized: "empty.noStarredItemsText"))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                        Button {
                            loadStarredItems()
                        } label: {
                            Label(String(localized: "mobile.actions.refresh"), systemImage: "arrow.clockwise")
                        }
                        .buttonStyle(.bordered)
                    }
                    .padding(.vertical, 8)
                } else if visibleItems.isEmpty {
                    Text(String(localized: "mobile.mail.noSearchMatches"))
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(visibleItems, id: \.id) { item in
                        StarredItemRow(item: item, showSenderImages: showSenderImages) {
                            readStarredItem(item)
                        } onToggleRead: {
                            markStarredItemRead(item, seen: item.unread)
                        } onUnstar: {
                            unstarStarredItem(item)
                        } onDelete: {
                            deleteStarredMailItem(item)
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .searchable(
            text: $starredSearch,
            placement: .navigationBarDrawer(displayMode: .always),
            prompt: Text(String(localized: "mobile.mail.searchCachedMail"))
        )
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    loadStarredItems()
                } label: {
                    Label(String(localized: "mobile.actions.refreshStarred"), systemImage: "arrow.clockwise")
                }
            }
        }
        .onAppear {
            if starredItems.isEmpty {
                loadStarredItems()
            }
        }
    }

    var filteredStarredItems: [StarredItemSummary] {
        kanbanStarredItemsMatching(starredItems, query: starredSearch)
    }
}
