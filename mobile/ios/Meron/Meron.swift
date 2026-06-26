import SwiftUI

@main
struct Meron: App {
    @AppStorage(iosAppearanceModeKey) private var appearanceMode = "system"
    @AppStorage(iosAppLanguageTagKey) private var appLanguageTag = ""
    @UIApplicationDelegateAdaptor(IosNotificationDelegate.self) private var notificationDelegate

    init() {
        IosBackgroundRefresh.register()
        IosBackgroundRefresh.schedule()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(\.locale, iosAppLocale(appLanguageTag))
                .preferredColorScheme(iosPreferredColorScheme(appearanceMode))
                .tint(iosThemeTint(appearanceMode))
                .id(iosNormalizedAppLanguageTag(appLanguageTag))
        }
    }
}
