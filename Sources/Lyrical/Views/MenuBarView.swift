import SwiftUI

struct MenuBarView: View {
    @Environment(NowPlayingModel.self) private var model
    @Environment(WindowManager.self) private var windows
    @Environment(\.openSettings) private var openSettings

    @AppStorage(Prefs.panelLocked) private var panelLocked = false
    @AppStorage(Prefs.shazamEnabled) private var shazamEnabled = true
    @AppStorage(Prefs.syncOffset) private var syncOffset = 0.0

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            NowPlayingCard()

            AdaptiveGlassContainer(spacing: 10) {
                HStack(spacing: 10) {
                    actionTile(
                        title: "Floating", symbol: "pip.fill",
                        isOn: windows.isFloatingVisible
                    ) { windows.setFloating(!windows.isFloatingVisible) }
                    actionTile(title: "Full Screen", symbol: "arrow.up.left.and.arrow.down.right", isOn: false) {
                        windows.showFullScreen()
                    }
                }
            }

            VStack(alignment: .leading, spacing: 10) {
                Toggle(isOn: $panelLocked) {
                    Label("Click-through floating lyrics", systemImage: "lock")
                }
                Toggle(isOn: $shazamEnabled) {
                    Label("Identify audio from any app", systemImage: "shazam.logo")
                }
                if shazamEnabled {
                    HStack(spacing: 6) {
                        Circle().fill(model.shazamStatus.tint).frame(width: 6, height: 6)
                        Text(model.shazamStatus.label)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                        if model.shazamStatus == .permissionNeeded {
                            Spacer()
                            Button("Open Settings") { openPrivacySettings() }
                                .controlSize(.small)
                        }
                    }
                    .padding(.leading, 28)
                }
                ForEach(model.blockedBrowsers, id: \.self) { browser in
                    BrowserPermissionHint(source: browser)
                }
                HStack {
                    Label("Lyrics timing", systemImage: "timer")
                    Spacer()
                    Text(syncOffset == 0 ? "In sync" : String(format: "%+.2fs", syncOffset))
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                    Stepper("", value: $syncOffset, in: -5...5, step: 0.25)
                        .labelsHidden()
                }
            }
            .toggleStyle(.switch)
            .controlSize(.small)
            .onChange(of: panelLocked) { _, locked in windows.applyLock(locked) }
            .onChange(of: shazamEnabled) { _, enabled in model.setShazamEnabled(enabled) }

            Divider()

            HStack {
                Button {
                    NSApp.activate()
                    openSettings()
                } label: {
                    Label("Settings…", systemImage: "gearshape")
                }
                Spacer()
                Button {
                    NSApp.terminate(nil)
                } label: {
                    Label("Quit", systemImage: "power")
                }
            }
            .buttonStyle(.borderless)
            .foregroundStyle(.secondary)
        }
        .padding(16)
        .frame(width: 330)
    }

    private func actionTile(title: String, symbol: String, isOn: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 6) {
                Image(systemName: symbol).font(.system(size: 17, weight: .semibold))
                Text(title).font(.system(size: 11, weight: .semibold))
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
            .foregroundStyle(isOn ? .white : .primary)
            .contentShape(RoundedRectangle(cornerRadius: 16))
        }
        .buttonStyle(.plain)
        .adaptiveGlass(isOn ? .regular.tint(model.accent).interactive() : .regular.interactive(),
                     in: .rect(cornerRadius: 16, style: .continuous))
    }

    private func openPrivacySettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
            NSWorkspace.shared.open(url)
        }
    }
}

private struct NowPlayingCard: View {
    @Environment(NowPlayingModel.self) private var model
    @AppStorage(Prefs.syncOffset) private var syncOffset = 0.0

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                ArtworkView(image: model.artwork, cornerRadius: 10, palette: model.palette)
                    .frame(width: 56, height: 56)
                    .shadow(color: .black.opacity(0.2), radius: 6, y: 3)
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.track?.title ?? "Nothing Playing")
                        .font(.system(size: 14, weight: .semibold))
                        .lineLimit(1)
                    Text(model.track?.artist ?? "Play music in any app")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    if let source = model.source {
                        Label(source.title, systemImage: source.symbol)
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(.tertiary)
                    }
                }
                Spacer(minLength: 0)
                if model.canControl {
                    HStack(spacing: 2) {
                        Button { model.togglePlayPause() } label: {
                            Image(systemName: model.isPlaying ? "pause.fill" : "play.fill")
                                .contentTransition(.symbolEffect(.replace))
                                .frame(width: 28, height: 28)
                        }
                        Button { model.next() } label: {
                            Image(systemName: "forward.fill").frame(width: 28, height: 28)
                        }
                    }
                    .buttonStyle(.borderless)
                    .font(.system(size: 14))
                }
            }

            if let lines = model.lyrics.syncedLines {
                TimelineView(.periodic(from: .now, by: 0.25)) { context in
                    let index = lines.activeIndex(at: model.time(at: context.date) + syncOffset)
                    let text = index.map { lines[$0].text } ?? ""
                    Text(text.isEmpty ? "♪" : text)
                        .font(.system(size: 13, weight: .semibold))
                        .lineLimit(2)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentTransition(.opacity)
                        .animation(.easeOut(duration: 0.25), value: index)
                }
            }
        }
        .padding(12)
        .adaptiveGlass(.regular.tint(model.accent.opacity(0.25)), in: .rect(cornerRadius: 20, style: .continuous))
    }
}

/// Shown when a YouTube Music tab is open but the browser blocks Apple Events JavaScript.
private struct BrowserPermissionHint: View {
    let source: PlaybackSource

    private var steps: String {
        source == .youTubeMusicChrome
            ? "In Chrome, choose View › Developer › Allow JavaScript from Apple Events."
            : "In Safari, turn on Settings › Advanced › Show features for web developers, then choose Develop › Allow JavaScript from Apple Events."
    }

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
            VStack(alignment: .leading, spacing: 2) {
                Text("YouTube Music needs one setting for exact sync")
                    .font(.caption.weight(.semibold))
                Text(steps)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(10)
        .adaptiveGlass(.regular.tint(.orange.opacity(0.15)), in: .rect(cornerRadius: 12, style: .continuous))
    }
}
