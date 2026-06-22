import CryptoKit
import MeronShared
import SwiftUI
import UniformTypeIdentifiers

struct ContentView: View {
    private let protocolVersion = CoreProtocolKt.EXPECTED_PROTOCOL_VERSION
    private let rustInitJson: String
    private let rustProtocolVersion = RustCoreBridge.protocolVersion()
    private let rustPingJson = RustCoreBridge.pingJson()
    private let rustReadyEvents = RustCoreBridge.readyEvents()
    private let threadListJson: String
    @State private var mailtoDraft: ComposeDraft?
    @State private var attachments: [DraftAttachment] = []
    @State private var isFileImporterPresented = false
    @State private var attachmentError: String?
    @State private var backgroundRefreshStatus = "Scheduled on app launch."
    @State private var notificationStatus = "Not requested."
    @State private var accountEmail = "user1@mail.localhost"
    @State private var accountPassword = "user1password"
    @State private var accountDisplayName = "Local Test"
    @State private var accountSenderName = "Local Test"
    @State private var imapHost = "10.0.2.2"
    @State private var imapPort = "993"
    @State private var smtpHost = "10.0.2.2"
    @State private var smtpPort = "465"
    @State private var oauthProvider = "gmail"
    @State private var oauthEmail = "me@gmail.com"
    @State private var oauthAccessToken = ""
    @State private var oauthRefreshToken = ""
    @State private var oauthExpiresAt = "0"
    @State private var oauthClientId = ""
    @State private var oauthClientSecret = ""
    @State private var oauthRedirectUri = OAuthFlowKt.defaultOAuthRedirectUri()
    @State private var oauthState = UUID().uuidString
    @State private var oauthVerifier = UUID().uuidString + UUID().uuidString
    @State private var oauthAuthorizationCode = ""
    @State private var rssFeedUrl = "https://example.com/feed.xml"
    @State private var rssDisplayName = "Example Feed"
    @State private var accountStatus = "Not loaded."
    @State private var accountJson = ""
    @State private var coreAccounts: [AccountSummary] = []
    @State private var selectedCoreAccountId = ""
    @State private var coreFolders: [FolderSummary] = []
    @State private var selectedCoreFolder = "inbox"
    @State private var coreThreads: [ThreadSummary] = []
    @State private var selectedCoreThread: ThreadSummary?
    @State private var coreMessages: [MessageBody] = []
    @State private var composeTo = "user1@mail.localhost"
    @State private var composeCc = ""
    @State private var composeBcc = ""
    @State private var composeSubject = "Hello from Meron iOS"
    @State private var composeBody = "This message was sent from the native iOS shell through meron-core."
    @State private var quickReplyBody = ""

    init() {
        rustInitJson = RustCoreBridge.initJson(dataDirectory: IosAppPaths.mobileDataDirectory())
        let params = ThreadListParams(
            accountId: "mobile-demo",
            folderId: "inbox",
            query: "",
            filter: "all",
            beforeCursor: nil,
            refresh: false
        )
        threadListJson = MobileCommandsKt.threadListRequest(id: 1, params: params).toJson()
    }

    var body: some View {
        TabView {
            NavigationStack {
                mailView
                    .navigationTitle(selectedCoreFolder.isEmpty ? "Inbox" : selectedCoreFolder.capitalized)
                    .navigationBarTitleDisplayMode(.inline)
            }
            .tabItem { Label("Mail", systemImage: "tray") }

            NavigationStack {
                composeView
                    .navigationTitle("Compose")
                    .navigationBarTitleDisplayMode(.inline)
            }
            .tabItem { Label("Compose", systemImage: "square.and.pencil") }

            NavigationStack {
                accountsView
                    .navigationTitle("Accounts")
                    .navigationBarTitleDisplayMode(.inline)
            }
            .tabItem { Label("Accounts", systemImage: "person.crop.circle") }
        }
        .onOpenURL { url in
            if OAuthFlowKt.isOAuthCallbackUrl(rawUrl: url.absoluteString, redirectUri: oauthRedirectUri) {
                handleOAuthCallback(url)
            } else {
                mailtoDraft = MailtoKt.parseMailtoUrl(rawUrl: url.absoluteString)
                if let draft = mailtoDraft {
                    applyMailtoDraftToCompose(
                        draft,
                        to: &composeTo,
                        cc: &composeCc,
                        bcc: &composeBcc,
                        subject: &composeSubject,
                        body: &composeBody
                    )
                }
            }
        }
        .fileImporter(isPresented: $isFileImporterPresented, allowedContentTypes: [.item]) { result in
            switch result {
            case .success(let url):
                addAttachment(from: url)
            case .failure(let error):
                attachmentError = error.localizedDescription
            }
        }
    }

    private var mailView: some View {
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
                Section("Mailbox") {
                    Picker("Account", selection: $selectedCoreAccountId) {
                        ForEach(coreAccounts, id: \.id) { account in
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
                    }

                    if !coreFolders.isEmpty {
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

                    Button {
                        syncSelectedAccount()
                    } label: {
                        Label("Sync Mailbox", systemImage: "arrow.clockwise")
                    }
                }
            }

            Section("Inbox") {
                if coreThreads.isEmpty {
                    Text("Sync the selected mailbox to load cached threads.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(coreThreads, id: \.id) { thread in
                        ThreadRow(thread: thread) {
                            readThread(thread)
                        } actions: {
                            Button(thread.unread ? "Mark Read" : "Mark Unread") {
                                markThreadRead(thread, seen: thread.unread)
                            }
                            Button(thread.starred ? "Unstar" : "Star") {
                                markThreadStarred(thread, starred: !thread.starred)
                            }
                            if isRssThread(thread) {
                                Button("Remove Feed", role: .destructive) {
                                    removeRssFeed(thread)
                                }
                            } else {
                                Button("Archive") {
                                    archiveThread(thread)
                                }
                                Button("Delete", role: .destructive) {
                                    deleteThread(thread)
                                }
                            }
                        }
                    }
                }
            }

            Section("Conversation") {
                if let selectedCoreThread {
                    Text(selectedCoreThread.subject.isEmpty ? selectedCoreThread.id : selectedCoreThread.subject)
                        .font(.headline)
                }

                if coreMessages.isEmpty {
                    Text("Open a thread to read cached messages.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(coreMessages, id: \.id) { message in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(message.subject.isEmpty ? "(no subject)" : message.subject)
                                .font(.headline)
                            Text(message.from)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                            Text(message.body)
                                .font(.body)
                                .textSelection(.enabled)
                        }
                        .padding(.vertical, 4)
                    }

                    if let selectedCoreThread, !isRssThread(selectedCoreThread) {
                        TextEditor(text: $quickReplyBody)
                            .frame(minHeight: 90)
                        Button {
                            sendQuickReply()
                        } label: {
                            Label("Send Reply", systemImage: "arrowshape.turn.up.left")
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    private var composeView: some View {
        List {
            Section {
                TextField("To", text: $composeTo)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Cc", text: $composeCc)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Bcc", text: $composeBcc)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Subject", text: $composeSubject)
                TextEditor(text: $composeBody)
                    .frame(minHeight: 220)
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
                    sendCoreMail()
                } label: {
                    Label("Send Message", systemImage: "paperplane.fill")
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    private var accountsView: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text(accountStatus)
                        .font(.headline)
                    Text("Manage providers, background refresh, and core diagnostics.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            Section("Background Refresh") {
                Button {
                    IosBackgroundRefresh.runOnce { summary in
                        backgroundRefreshStatus = summary
                    }
                } label: {
                    Label("Run Background Refresh", systemImage: "arrow.clockwise")
                }
                Button {
                    IosNotificationService.requestAuthorization { granted in
                        notificationStatus = granted ? "Notifications enabled." : "Notifications disabled."
                    }
                } label: {
                    Label("Enable Notifications", systemImage: "bell")
                }
                LabeledContent("Refresh", value: backgroundRefreshStatus)
                LabeledContent("Notifications", value: notificationStatus)
            }

            Section("Password Account") {
                TextField("Email", text: $accountEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField("Password", text: $accountPassword)
                TextField("Display name", text: $accountDisplayName)
                TextField("Sender name", text: $accountSenderName)
                TextField("IMAP host", text: $imapHost)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("IMAP port", text: $imapPort)
                    .keyboardType(.numberPad)
                TextField("SMTP host", text: $smtpHost)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("SMTP port", text: $smtpPort)
                    .keyboardType(.numberPad)
                Button {
                    addPasswordAccount()
                } label: {
                    Label("Add Password Account", systemImage: "person.badge.plus")
                }
                Button {
                    listAccounts()
                } label: {
                    Label("Reload Accounts", systemImage: "list.bullet")
                }
            }

            Section("OAuth") {
                Picker("Provider", selection: $oauthProvider) {
                    Text("Gmail").tag("gmail")
                    Text("Outlook").tag("outlook")
                }
                .pickerStyle(.segmented)
                TextField("OAuth email", text: $oauthEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("OAuth client ID", text: $oauthClientId)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField("OAuth client secret (optional)", text: $oauthClientSecret)
                TextField("Redirect URI", text: $oauthRedirectUri)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                Button {
                    launchOAuthFlow()
                } label: {
                    Label("Open OAuth in Browser", systemImage: "safari")
                }
                if !oauthAuthorizationCode.isEmpty {
                    LabeledContent("Authorization Code", value: oauthAuthorizationCode)
                }
                Button {
                    exchangeOAuthCode()
                } label: {
                    Label("Exchange Code And Add Account", systemImage: "arrow.triangle.2.circlepath")
                }
                TextField("Access token", text: $oauthAccessToken)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField("Refresh token", text: $oauthRefreshToken)
                TextField("Token expires at", text: $oauthExpiresAt)
                    .keyboardType(.numberPad)
                Button {
                    addOAuthAccount()
                } label: {
                    Label("Add OAuth Account", systemImage: "person.crop.circle.badge.checkmark")
                }
            }

            Section("RSS") {
                TextField("RSS feed URL", text: $rssFeedUrl)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("RSS name", text: $rssDisplayName)
                Button {
                    addRssAccount()
                } label: {
                    Label("Add RSS Account", systemImage: "dot.radiowaves.left.and.right")
                }
            }

            Section("Diagnostics") {
                DisclosureGroup("Core contract") {
                    LabeledContent("Expected protocol", value: "\(protocolVersion)")
                    LabeledContent("Rust protocol", value: "\(rustProtocolVersion)")
                    LabeledContent("Command", value: MobileCommand.shared.ThreadList)
                    DiagnosticText(title: "Init", value: rustInitJson)
                    DiagnosticText(title: "Ping", value: rustPingJson)
                    DiagnosticText(title: "Generated request", value: threadListJson)
                    ForEach(rustReadyEvents, id: \.self) { event in
                        DiagnosticText(title: "Event", value: event)
                    }
                }
                if !accountJson.isEmpty {
                    DisclosureGroup("Last core response") {
                        Text(accountJson)
                            .font(.system(.footnote, design: .monospaced))
                            .textSelection(.enabled)
                    }
                }
                Label("Use registered Gmail/Outlook mobile client IDs and exact redirect URIs.", systemImage: "safari")
            }
        }
        .listStyle(.insetGrouped)
    }

    private func listAccounts() {
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 10).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = "Loaded accounts."
    }

    private func addPasswordAccount() {
        let params = AddPasswordAccountParams(
            email: accountEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            imapHost: imapHost.trimmingCharacters(in: .whitespacesAndNewlines),
            imapPort: Int32(imapPort) ?? 993,
            smtpHost: smtpHost.trimmingCharacters(in: .whitespacesAndNewlines),
            smtpPort: Int32(smtpPort) ?? 465,
            username: accountEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            password: accountPassword,
            tls: true
        )
        let request = MobileCommandsKt.accountAddPasswordRequest(id: 11, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        accountStatus = response.contains(#""error""#) ? "Password account failed." : "Password account added."
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 12).toJson())
        updateCoreAccounts(from: accountJson)
    }

    private func addRssAccount() {
        let params = AddRssAccountParams(
            feedUrl: rssFeedUrl.trimmingCharacters(in: .whitespacesAndNewlines),
            displayName: rssDisplayName.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        let request = MobileCommandsKt.accountAddRssRequest(id: 13, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        accountStatus = response.contains(#""error""#) ? "RSS account failed." : "RSS account added."
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 14).toJson())
        updateCoreAccounts(from: accountJson)
    }

    private func addOAuthAccount() {
        let refreshToken = oauthRefreshToken.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !refreshToken.isEmpty else {
            accountStatus = "OAuth refresh token is required."
            return
        }
        let params = AddOAuthAccountParams(
            email: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            provider: oauthProvider,
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            username: "",
            avatarUrl: "",
            accessToken: oauthAccessToken.trimmingCharacters(in: .whitespacesAndNewlines),
            refreshToken: refreshToken,
            tokenExpiresAt: Int64(oauthExpiresAt) ?? 0,
            imapHost: "",
            imapPort: nil,
            smtpHost: "",
            smtpPort: nil
        )
        let request = MobileCommandsKt.accountAddOAuthRequest(id: 23, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        accountStatus = response.contains(#""error""#) ? "OAuth account failed." : "OAuth account added."
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 24).toJson())
        updateCoreAccounts(from: accountJson)
    }

    private func launchOAuthFlow() {
        let clientId = oauthClientId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clientId.isEmpty else {
            accountStatus = "OAuth client ID is required."
            return
        }
        oauthState = UUID().uuidString
        oauthVerifier = UUID().uuidString + UUID().uuidString
        let params = OAuthAuthorizationRequest(
            provider: oauthProvider,
            clientId: clientId,
            redirectUri: oauthRedirectUri.trimmingCharacters(in: .whitespacesAndNewlines),
            state: oauthState,
            codeChallenge: pkceChallenge(oauthVerifier),
            loginHint: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        guard let url = URL(string: OAuthFlowKt.buildOAuthAuthorizationUrl(request: params)) else {
            accountStatus = "OAuth URL could not be built."
            return
        }
        UIApplication.shared.open(url)
        accountStatus = "Opened \(oauthProvider) OAuth in browser."
    }

    private func exchangeOAuthCode() {
        let code = oauthAuthorizationCode.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else {
            accountStatus = "OAuth authorization code is required."
            return
        }
        let clientId = oauthClientId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clientId.isEmpty else {
            accountStatus = "OAuth client ID is required."
            return
        }
        let params = ExchangeOAuthCodeParams(
            email: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            provider: oauthProvider,
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            code: code,
            clientId: clientId,
            clientSecret: oauthClientSecret.trimmingCharacters(in: .whitespacesAndNewlines),
            redirectUri: oauthRedirectUri.trimmingCharacters(in: .whitespacesAndNewlines),
            codeVerifier: oauthVerifier
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.accountExchangeOAuthCodeRequest(id: 25, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = "OAuth exchange failed."
            accountJson = response
            return
        }
        accountStatus = "OAuth account added."
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 26).toJson())
        updateCoreAccounts(from: accountJson)
    }

    private func handleOAuthCallback(_ url: URL) {
        accountStatus = applyOAuthCallbackToComposeState(
            rawUrl: url.absoluteString,
            expectedState: oauthState,
            redirectUri: oauthRedirectUri,
            authorizationCode: &oauthAuthorizationCode
        )
    }

    private func updateCoreAccounts(from response: String) {
        coreAccounts = MobileResponseParsersKt.parseAccountListResponse(responseJson: response)
        if !coreAccounts.contains(where: { $0.id == selectedCoreAccountId }) {
            selectedCoreAccountId = coreAccounts.first?.id ?? ""
        }
    }

    private func syncSelectedAccount() {
        let accountId = selectedCoreAccountId.isEmpty ? (coreAccounts.first?.id ?? "") : selectedCoreAccountId
        guard !accountId.isEmpty else {
            accountStatus = "No account selected."
            return
        }

        let syncResponse: String
        let requestedFolder = selectedCoreFolder.isEmpty ? "inbox" : selectedCoreFolder
        if selectedAccountIsRss(accountId) {
            let syncParams = SyncRssParams(accountId: accountId)
            syncResponse = RustCoreBridge.invokeJson(MobileCommandsKt.syncRssRequest(id: 15, params: syncParams).toJson())
        } else {
            let syncParams = SyncMailParams(accountId: accountId, folderId: requestedFolder, limit: 50, folders: true)
            syncResponse = RustCoreBridge.invokeJson(MobileCommandsKt.syncMailRequest(id: 15, params: syncParams).toJson())
        }
        if syncResponse.contains(#""error""#) {
            accountStatus = "Core sync failed."
            accountJson = syncResponse
            return
        }

        loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: requestedFolder)
    }

    private func loadCoreFoldersAndThreads(accountId: String, requestedFolder: String? = nil) {
        let folderResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 14, params: FolderListParams(accountId: accountId)).toJson()
        )
        let parsedFolders = MobileResponseParsersKt.parseFolderListResponse(responseJson: folderResponse)
        coreFolders = parsedFolders
        let fallbackFolder = parsedFolders.first?.name ?? "inbox"
        let folderId = parsedFolders.contains(where: { $0.name == (requestedFolder ?? selectedCoreFolder) })
            ? (requestedFolder ?? selectedCoreFolder)
            : fallbackFolder
        selectedCoreFolder = folderId
        loadCoreThreads(accountId: accountId, folderId: folderId)
    }

    private func loadCoreThreads(accountId: String, folderId: String) {
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: folderId,
            query: "",
            filter: "all",
            beforeCursor: nil,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 16, params: threadParams).toJson())
        coreThreads = MobileResponseParsersKt.parseThreadListResponse(responseJson: threadResponse)
        accountJson = threadResponse
        if let selectedCoreThread, !coreThreads.contains(where: { $0.id == selectedCoreThread.id }) {
            self.selectedCoreThread = nil
            coreMessages = []
        }
        accountStatus = "Loaded \(coreThreads.count) core thread(s) from \(folderId)."
    }

    private func readThread(_ thread: ThreadSummary) {
        selectedCoreThread = thread
        let response: String
        if isRssThread(thread) {
            let params = RssThreadParams(threadId: thread.id, beforeCursor: nil, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.rssThreadRequest(id: 21, params: params).toJson())
        } else {
            let params = ThreadReadParams(threadId: thread.id, beforeCursor: nil, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.threadReadRequest(id: 21, params: params).toJson())
        }
        if response.contains(#""error""#) {
            accountStatus = "Thread read failed."
            accountJson = response
            return
        }
        coreMessages = MobileResponseParsersKt.parseThreadReadResponse(responseJson: response)
        accountJson = response
        accountStatus = "Loaded \(coreMessages.count) message(s)."
    }

    private func sendCoreMail() {
        let accountId = selectedCoreAccountId.isEmpty ? (coreAccounts.first?.id ?? "") : selectedCoreAccountId
        guard !accountId.isEmpty else {
            accountStatus = "Select or add an account before sending."
            return
        }
        guard !composeTo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !composeSubject.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !composeBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            accountStatus = "Complete To, Subject, and Body before sending."
            return
        }
        let attachmentInputs = attachments.map {
            MobileAttachmentInput(filename: $0.displayName, mime: $0.mimeType, data: $0.dataBase64, inlineId: "")
        }
        let params = SendMailParams(
            accountId: accountId,
            to: composeTo.trimmingCharacters(in: .whitespacesAndNewlines),
            subject: composeSubject.trimmingCharacters(in: .whitespacesAndNewlines),
            body: composeBody.trimmingCharacters(in: .whitespacesAndNewlines),
            from: "",
            cc: composeCc.trimmingCharacters(in: .whitespacesAndNewlines),
            bcc: composeBcc.trimmingCharacters(in: .whitespacesAndNewlines),
            replyTo: "",
            html: "",
            inReplyTo: "",
            references: "",
            messageId: "",
            attachments: attachmentInputs
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.sendMailRequest(id: 22, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = "Core send failed."
            accountJson = response
            return
        }
        attachments = []
        accountStatus = "Sent through core."
        loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
    }

    private func sendQuickReply() {
        let accountId = selectedCoreAccountId.isEmpty ? (coreAccounts.first?.id ?? "") : selectedCoreAccountId
        guard !accountId.isEmpty, let selectedCoreThread, let parent = coreMessages.last else {
            accountStatus = "Open a mail thread before replying."
            return
        }
        guard !isRssThread(selectedCoreThread) else {
            accountStatus = "RSS threads do not support replies."
            return
        }
        let body = quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else {
            accountStatus = "Write a reply before sending."
            return
        }

        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.sendMailRequest(id: 27, params: parent.toReplyMailParams(accountId: accountId, body: body)).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Quick reply failed."
            accountJson = response
            return
        }
        quickReplyBody = ""
        accountStatus = "Quick reply sent."
    }

    private func archiveThread(_ thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: nil, messageIds: [])
        runThreadAction(
            requestJson: MobileCommandsKt.archiveThreadRequest(id: 17, params: params).toJson(),
            successStatus: "Archive complete."
        )
    }

    private func deleteThread(_ thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        runThreadAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 18, params: params).toJson(),
            successStatus: "Delete complete."
        )
    }

    private func removeRssFeed(_ thread: ThreadSummary) {
        let params = RemoveRssFeedParams(threadId: thread.id)
        runThreadAction(
            requestJson: MobileCommandsKt.feedRemoveRequest(id: 18, params: params).toJson(),
            successStatus: "Removed feed."
        )
    }

    private func markThreadRead(_ thread: ThreadSummary, seen: Bool) {
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkReadParams(threadId: thread.id, seen: seen, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkReadRequest(id: 19, params: params).toJson()
        } else {
            let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [])
            requestJson = MobileCommandsKt.markReadRequest(id: 19, params: params).toJson()
        }
        runThreadAction(requestJson: requestJson, successStatus: seen ? "Marked read." : "Marked unread.")
    }

    private func markThreadStarred(_ thread: ThreadSummary, starred: Bool) {
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkStarredParams(threadId: thread.id, starred: starred, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkStarredRequest(id: 20, params: params).toJson()
        } else {
            let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [])
            requestJson = MobileCommandsKt.markStarredRequest(id: 20, params: params).toJson()
        }
        runThreadAction(requestJson: requestJson, successStatus: starred ? "Starred." : "Unstarred.")
    }

    private func runThreadAction(requestJson: String, successStatus: String) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Thread action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        if !selectedCoreAccountId.isEmpty {
            loadCoreFoldersAndThreads(accountId: selectedCoreAccountId, requestedFolder: selectedCoreFolder)
        }
    }

    private func selectedAccountIsRss(_ accountId: String) -> Bool {
        guard let account = coreAccounts.first(where: { $0.id == accountId }) else {
            return false
        }
        return MailStateKt.accountSummaryIsRss(account: account)
    }

    private func isRssThread(_ thread: ThreadSummary) -> Bool {
        MailStateKt.threadIdIsRss(threadId: thread.id)
    }

    private func addAttachment(from url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            attachments.append(
                DraftAttachment(
                    id: url.absoluteString,
                    displayName: url.lastPathComponent,
                    mimeType: type,
                    sizeBytes: Int64(data.count),
                    dataBase64: data.base64EncodedString()
                )
            )
            attachmentError = nil
        } catch {
            attachmentError = error.localizedDescription
        }
    }
}

private struct ThreadRow<Actions: View>: View {
    let thread: ThreadSummary
    let onOpen: () -> Void
    @ViewBuilder let actions: () -> Actions

    var body: some View {
        Button(action: onOpen) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Text(thread.sender.isEmpty ? thread.accountId : thread.sender)
                        .font(.subheadline.weight(thread.unread ? .semibold : .regular))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    Spacer()
                    if thread.unread {
                        Text("Unread")
                            .font(.caption2.weight(.bold))
                            .foregroundStyle(.tint)
                    }
                    if thread.starred {
                        Image(systemName: "star.fill")
                            .font(.caption)
                            .foregroundStyle(.yellow)
                    }
                }
                Text(thread.subject.isEmpty ? "(no subject)" : thread.subject)
                    .font(.headline)
                    .foregroundStyle(.primary)
                    .lineLimit(2)
                if !thread.preview.isEmpty {
                    Text(thread.preview)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
                Text(thread.folder)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            actions()
        }
    }
}

private struct DiagnosticText: View {
    let title: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(.footnote, design: .monospaced))
                .textSelection(.enabled)
        }
    }
}

func applyMailtoDraftToCompose(
    _ draft: ComposeDraft,
    to: inout String,
    cc: inout String,
    bcc: inout String,
    subject: inout String,
    body: inout String
) {
    to = draft.to
    cc = draft.cc
    bcc = draft.bcc
    subject = draft.subject
    body = draft.body
}

func applyOAuthCallbackToComposeState(
    rawUrl: String,
    expectedState: String,
    redirectUri: String = OAuthFlowKt.defaultOAuthRedirectUri(),
    authorizationCode: inout String
) -> String {
    if let callback = OAuthFlowKt.parseOAuthCallbackUrlForRedirectOrNull(
        rawUrl: rawUrl,
        expectedState: expectedState,
        redirectUri: redirectUri
    ) {
        authorizationCode = callback.code
        return "OAuth authorization code received; use Exchange Code And Add Account."
    }

    let error = OAuthFlowKt.oauthCallbackValidationErrorForRedirect(
        rawUrl: rawUrl,
        expectedState: expectedState,
        redirectUri: redirectUri
    ) ?? "OAuth callback failed."
    return "OAuth callback failed: \(error)"
}

private func pkceChallenge(_ verifier: String) -> String {
    let digest = SHA256.hash(data: Data(verifier.utf8))
    return Data(digest)
        .base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}

#Preview {
    ContentView()
}
