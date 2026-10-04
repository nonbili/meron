import AppIntents
import SwiftUI
import WidgetKit

struct MailAccount: AppEntity {
    static var typeDisplayRepresentation: TypeDisplayRepresentation = "settings.account.account"
    static var defaultQuery = MailAccountQuery()
    let id: String
    let title: String
    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(title)")
    }

    static func available() -> [MailAccount] {
        let unified = MailAccount(id: "unified", title: String(localized: "accounts.unified"))
        return [unified] + (UnreadWidgetStore.read()?.scopes.filter { $0.id != "unified" }.map { MailAccount(id: $0.id, title: $0.title) } ?? [])
    }
}

struct MailAccountQuery: EntityQuery {
    func entities(for identifiers: [String]) async throws -> [MailAccount] {
        MailAccount.available().filter { identifiers.contains($0.id) }
    }

    func suggestedEntities() async throws -> [MailAccount] {
        MailAccount.available()
    }

    func defaultResult() async -> MailAccount? {
        MailAccount.available().first
    }
}

struct MailWidgetConfiguration: WidgetConfigurationIntent {
    static var title: LocalizedStringResource = "mobile.ios.widgetName"
    @Parameter(title: "settings.account.account") var account: MailAccount?
}

struct MailEntry: TimelineEntry {
    let date: Date
    let snapshot: UnreadWidgetSnapshot?
    let accountId: String?
    var scope: UnreadWidgetScope? {
        snapshot?.scope(accountId)
    }
}

struct MailTimelineProvider: AppIntentTimelineProvider {
    func placeholder(in _: Context) -> MailEntry {
        MailEntry(date: Date(), snapshot: UnreadWidgetSnapshot(updatedAt: Date(), scopes: [UnreadWidgetScope(id: "unified", title: "", count: 3, threads: [UnreadWidgetThread(id: "preview", sender: "Meron", subject: String(localized: "mobile.ios.widgetName"), date: Date().timeIntervalSince1970)])]), accountId: nil)
    }

    func snapshot(for configuration: MailWidgetConfiguration, in context: Context) async -> MailEntry {
        context.isPreview ? placeholder(in: context) : entry(configuration)
    }

    func timeline(for configuration: MailWidgetConfiguration, in _: Context) async -> Timeline<MailEntry> {
        Timeline(entries: [entry(configuration)], policy: .after(Date().addingTimeInterval(1800)))
    }

    private func entry(_ configuration: MailWidgetConfiguration) -> MailEntry {
        MailEntry(date: Date(), snapshot: UnreadWidgetStore.read(), accountId: configuration.account?.id)
    }
}

struct MailWidgetView: View {
    let entry: MailEntry
    let showsList: Bool
    @Environment(\.widgetFamily) private var family

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Image(systemName: "envelope.badge")
                Text(entry.scope?.title.isEmpty == false ? entry.scope!.title : String(localized: "accounts.unified"))
                    .lineLimit(1)
                if showsList {
                    Spacer(minLength: 0)
                    Text(count).bold().privacySensitive()
                }
            }
            .font(.caption)
            if showsList {
                if let scope = entry.scope {
                    if scope.threads.isEmpty {
                        Spacer(minLength: 0)
                        Text("mobile.ios.widgetListEmpty").font(.subheadline)
                        Spacer(minLength: 0)
                    } else {
                        ForEach(scope.threads.prefix(family == .systemLarge ? 5 : 2)) { thread in
                            Link(destination: thread.url) {
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack {
                                        Text(thread.sender).font(.subheadline).bold().lineLimit(1)
                                        Spacer(minLength: 4)
                                        if thread.date > 0 {
                                            Text(thread.dateLabel(relativeTo: entry.date))
                                                .font(.caption2).foregroundStyle(.secondary)
                                                .lineLimit(1).fixedSize(horizontal: true, vertical: false)
                                        }
                                    }
                                    Text(thread.subject).font(.caption).lineLimit(1).foregroundStyle(.secondary)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .privacySensitive()
                            }
                        }
                        Spacer(minLength: 0)
                    }
                } else {
                    Spacer(minLength: 0)
                    Text("mobile.ios.widgetOpenApp").font(.subheadline)
                    Spacer(minLength: 0)
                }
            } else {
                Text(count).font(.system(size: 44, weight: .bold, design: .rounded)).minimumScaleFactor(0.6).lineLimit(1).privacySensitive()
                Text("mobile.ios.widgetName").font(.caption)
                Spacer(minLength: 0)
            }
            if let updated = entry.snapshot?.updatedAt {
                HStack(spacing: 3) {
                    Text("mobile.ios.widgetUpdated")
                    Text(updated, style: .relative)
                }.font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            } else if !showsList {
                Text("mobile.ios.widgetOpenApp").font(.caption2).foregroundStyle(.secondary)
            }
        }
        .widgetURL(URL(string: "meron://open")!)
        .containerBackground(.background, for: .widget)
    }

    private var count: String {
        guard let count = entry.scope?.count else { return "—" }
        return count > 999 ? "999+" : String(count)
    }
}

struct UnreadCountWidget: Widget {
    var body: some WidgetConfiguration {
        AppIntentConfiguration(kind: "MeronUnreadCount", intent: MailWidgetConfiguration.self, provider: MailTimelineProvider()) {
            MailWidgetView(entry: $0, showsList: false)
        }
        .configurationDisplayName("mobile.ios.widgetName")
        .description("mobile.ios.widgetDescription")
        .supportedFamilies([.systemSmall])
    }
}

struct UnreadListWidget: Widget {
    var body: some WidgetConfiguration {
        AppIntentConfiguration(kind: "MeronUnreadList", intent: MailWidgetConfiguration.self, provider: MailTimelineProvider()) {
            MailWidgetView(entry: $0, showsList: true)
        }
        .configurationDisplayName("mobile.ios.widgetListName")
        .description("mobile.ios.widgetListDescription")
        .supportedFamilies([.systemMedium, .systemLarge])
    }
}

@main
struct MeronWidgets: WidgetBundle {
    var body: some Widget {
        UnreadCountWidget()
        UnreadListWidget()
    }
}
