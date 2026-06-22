import SwiftUI

@main
struct Meron: App {
    init() {
        IosBackgroundRefresh.register()
        IosBackgroundRefresh.schedule()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
