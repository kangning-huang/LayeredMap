import SwiftUI

@main
struct NimbleMapCompanionApp: App {
    init() {
        PhoneFiles.runSelfTestIfAsked()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
