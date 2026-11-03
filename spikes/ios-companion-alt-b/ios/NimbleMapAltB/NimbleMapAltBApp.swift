import SwiftUI

@main
struct NimbleMapAltBApp: App {
    init() {
        PhoneFiles.runSelfTestIfAsked()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
