import SwiftUI

@main
struct NimbleMapAltDApp: App {
    init() {
        PhoneFiles.runSelfTestIfAsked()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
