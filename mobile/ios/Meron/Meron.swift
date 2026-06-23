import SwiftUI

@main
struct Meron: App {
    @AppStorage(iosAppearanceModeKey) private var appearanceMode = "system"

    init() {
        IosBackgroundRefresh.register()
        IosBackgroundRefresh.schedule()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .preferredColorScheme(iosPreferredColorScheme(appearanceMode))
                .tint(iosThemeTint(appearanceMode))
        }
    }
}
