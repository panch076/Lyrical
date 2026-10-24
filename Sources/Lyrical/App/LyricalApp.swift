import AppKit
import SwiftUI

@main
struct LyricalApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    var body: some Scene {
        MenuBarExtra {
            MenuBarView()
                .environment(NowPlayingModel.shared)
                .environment(WindowManager.shared)
        } label: {
            Image(systemName: "quote.bubble.fill")
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsView()
                .environment(NowPlayingModel.shared)
                .environment(WindowManager.shared)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        Prefs.register()
        NSApp.setActivationPolicy(.accessory)
        NowPlayingModel.shared.start()
        if UserDefaults.standard.bool(forKey: Prefs.floatingVisible) {
            WindowManager.shared.setFloating(true)
        }
    }
}
