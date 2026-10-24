import ServiceManagement
import SwiftUI

struct SettingsView: View {
    @Environment(NowPlayingModel.self) private var model
    @Environment(WindowManager.self) private var windows

    @AppStorage(Prefs.fontScale) private var fontScale = 1.0
    @AppStorage(Prefs.syncOffset) private var syncOffset = 0.0
    @AppStorage(Prefs.panelGlass) private var panelGlass = PanelGlass.regular.rawValue
    @AppStorage(Prefs.showArtwork) private var showArtwork = true
    @AppStorage(Prefs.showNextLine) private var showNextLine = true
    @AppStorage(Prefs.karaokeSweep) private var karaokeSweep = true
    @AppStorage(Prefs.shazamEnabled) private var shazamEnabled = true

    @State private var launchAtLogin = SMAppService.mainApp.status == .enabled

    var body: some View {
        Form {
            Section("Floating Lyrics") {
                Picker("Glass", selection: $panelGlass) {
                    ForEach(PanelGlass.allCases) { Text($0.title).tag($0.rawValue) }
                }
                .pickerStyle(.segmented)
                Toggle("Show album art", isOn: $showArtwork)
                Toggle("Show previous and next lines", isOn: $showNextLine)
                Toggle("Highlight lines as they're sung", isOn: $karaokeSweep)
                LabeledContent("Text size") {
                    HStack {
                        Image(systemName: "textformat.size.smaller")
                        Slider(value: $fontScale, in: 0.7...1.8)
                        Image(systemName: "textformat.size.larger")
                    }
                }
            }

            Section {
                LabeledContent("Lyrics timing") {
                    HStack {
                        Slider(value: $syncOffset, in: -3...3, step: 0.05)
                        Text(String(format: "%+.2fs", syncOffset))
                            .monospacedDigit()
                            .frame(width: 56, alignment: .trailing)
                        Button("Reset") { syncOffset = 0 }
                            .disabled(syncOffset == 0)
                    }
                }
            } header: {
                Text("Sync")
            } footer: {
                Text("Positive values show lyrics earlier, negative values later.")
            }

            Section {
                Toggle("Identify songs playing in any app", isOn: $shazamEnabled)
                    .onChange(of: shazamEnabled) { _, enabled in model.setShazamEnabled(enabled) }
                LabeledContent("Status") {
                    Text(model.shazamStatus.label).foregroundStyle(model.shazamStatus.tint)
                }
            } header: {
                Text("Recognition")
            } footer: {
                Text("Apple Music and Spotify are read directly for exact timing. Audio from any other app — browsers, video players — is identified with Shazam, which needs the Screen & System Audio Recording permission.")
            }

            Section("General") {
                Toggle("Open at login", isOn: $launchAtLogin)
                    .onChange(of: launchAtLogin) { _, enabled in
                        do {
                            if enabled { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
                        } catch {
                            launchAtLogin = SMAppService.mainApp.status == .enabled
                        }
                    }
                LabeledContent("Lyrics provider") {
                    Link("LRCLIB", destination: URL(string: "https://lrclib.net")!)
                }
            }
        }
        .formStyle(.grouped)
        .onChange(of: fontScale) { windows.updatePanelSizeLimits() }
        .onChange(of: showNextLine) { windows.updatePanelSizeLimits() }
        .onChange(of: showArtwork) { windows.updatePanelSizeLimits() }
        .frame(width: 500)
        .fixedSize(horizontal: false, vertical: true)
    }
}
