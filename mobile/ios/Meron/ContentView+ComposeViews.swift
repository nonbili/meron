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
                    Picker("From", selection: Binding(
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
                TextField("To", text: $composeTo)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeTo) { _, value in loadRecipientSuggestions(field: "to", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "to", value: composeTo) }
                contactSuggestionRows(field: "to")
                TextField("Cc", text: $composeCc)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeCc) { _, value in loadRecipientSuggestions(field: "cc", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "cc", value: composeCc) }
                contactSuggestionRows(field: "cc")
                TextField("Bcc", text: $composeBcc)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeBcc) { _, value in loadRecipientSuggestions(field: "bcc", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "bcc", value: composeBcc) }
                contactSuggestionRows(field: "bcc")
                TextField("Subject", text: $composeSubject)
                TextEditor(text: $composeBody)
                    .frame(minHeight: 220)
                    .onKeyPress(keys: [.return]) { press in
                        if sendShortcutMatches(press, mode: sendShortcutMode) {
                            sendCoreMail()
                            return .handled
                        }
                        return .ignored
                    }
            }

            Section("Attachments") {
                Button {
                    isFileImporterPresented = true
                } label: {
                    Label("Attach File", systemImage: "paperclip")
                }
                if let attachmentError {
                    Text(attachmentError)
                        .foregroundStyle(.red)
                }
                if attachments.isEmpty {
                    Text("No attachments selected.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(attachments, id: \.id) { attachment in
                        VStack(alignment: .leading) {
                            Text(attachment.displayName)
                            Text("\(attachment.sizeBytes) bytes")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    Button("Clear Attachments", role: .destructive) {
                        attachments = []
                    }
                }
            }

            Section {
                if let draft = mailtoDraft {
                    Text("Loaded mailto draft for \(draft.to)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Button {
                    saveComposeDraft()
                } label: {
                    Label("Save Draft", systemImage: "tray.and.arrow.down")
                }
                Button(role: .destructive) {
                    discardComposeDraft()
                } label: {
                    Label("Discard Draft", systemImage: "trash")
                }
                Button {
                    sendCoreMail()
                } label: {
                    Label("Send Message", systemImage: "paperplane.fill")
                }
            }
        }
        .listStyle(.insetGrouped)
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
                Button {
                    loadStarredItems()
                } label: {
                    Label("Refresh Starred", systemImage: "arrow.clockwise")
                }
            }

            Section("Items") {
                if starredItems.isEmpty {
                    Text("Star messages or feed items to collect them here.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(starredItems, id: \.id) { item in
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
        .onAppear {
            if starredItems.isEmpty {
                loadStarredItems()
            }
        }
    }
}
