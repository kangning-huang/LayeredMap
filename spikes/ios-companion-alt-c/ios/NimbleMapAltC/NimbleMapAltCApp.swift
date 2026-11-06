import SwiftUI

@main
struct NimbleMapAltCApp: App {
    init() {
        PhoneFiles.runSelfTestIfAsked()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
