import AppKit
import Observation
import SwiftUI

@MainActor
@Observable
final class WindowManager {
    static let shared = WindowManager()

    private(set) var isFloatingVisible = false
    private(set) var isFullScreenVisible = false

    @ObservationIgnored private var panel: FloatingPanel?
    @ObservationIgnored private var fullScreen: FullScreenWindow?
    @ObservationIgnored private var keyMonitor: Any?

    private var model: NowPlayingModel { .shared }

    // MARK: - Floating lyrics

    func setFloating(_ visible: Bool) {
        UserDefaults.standard.set(visible, forKey: Prefs.floatingVisible)
        isFloatingVisible = visible
        if visible {
            guard !isFullScreenVisible else { return }
            let panel = self.panel ?? makePanel()
            panel.alphaValue = 0
            panel.orderFrontRegardless()
            NSAnimationContext.runAnimationGroup { $0.duration = 0.25; panel.animator().alphaValue = 1 }
        } else if let panel {
            NSAnimationContext.runAnimationGroup({ $0.duration = 0.2; panel.animator().alphaValue = 0 }) {
                Task { @MainActor in if !self.isFloatingVisible { panel.orderOut(nil) } }
            }
        }
    }

    func applyLock(_ locked: Bool) {
        panel?.ignoresMouseEvents = locked
    }

    /// Recomputes the floating panel's minimum size from the current text size,
    /// line count and artwork settings, growing the panel if it no longer fits.
    func updatePanelSizeLimits() {
        guard let panel else { return }
        let defaults = UserDefaults.standard
        let minimum = FloatingMetrics.minimumSize(
            fontScale: defaults.double(forKey: Prefs.fontScale),
            rows: defaults.bool(forKey: Prefs.showNextLine) ? 3 : 1,
            showArtwork: defaults.bool(forKey: Prefs.showArtwork))
        panel.applyMinimumSize(minimum)
    }

    private func makePanel() -> FloatingPanel {
        let root = FloatingLyricsView()
            .environment(model)
            .environment(self)
        let panel = FloatingPanel(content: root)
        panel.ignoresMouseEvents = UserDefaults.standard.bool(forKey: Prefs.panelLocked)
        self.panel = panel
        updatePanelSizeLimits()
        return panel
    }

    // MARK: - Full screen

    func showFullScreen() {
        if let fullScreen {
            fullScreen.makeKeyAndOrderFront(nil)
            return
        }
        let screen = panel?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        let root = FullScreenLyricsView(onClose: { [weak self] in self?.hideFullScreen() })
            .environment(model)
            .environment(self)
        let window = FullScreenWindow(screen: screen, content: root)
        window.alphaValue = 0

        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
        NSApp.presentationOptions = [.autoHideMenuBar, .autoHideDock]
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.45
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            window.animator().alphaValue = 1
        }

        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.isFullScreenVisible else { return event }
            switch event.keyCode {
            case 53: self.hideFullScreen(); return nil              // esc
            case 49: self.model.togglePlayPause(); return nil       // space
            case 124 where event.modifierFlags.contains(.command): self.model.next(); return nil
            case 123 where event.modifierFlags.contains(.command): self.model.previous(); return nil
            default: return event
            }
        }

        fullScreen = window
        isFullScreenVisible = true
        panel?.orderOut(nil)
    }

    func hideFullScreen() {
        guard let window = fullScreen else { return }
        fullScreen = nil
        isFullScreenVisible = false
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
        NSCursor.unhide()

        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.3
            window.animator().alphaValue = 0
        }) {
            Task { @MainActor in
                window.orderOut(nil)
                NSApp.presentationOptions = []
                if self.isFloatingVisible { self.setFloating(true) }
            }
        }
    }

    func toggleFullScreen() {
        isFullScreenVisible ? hideFullScreen() : showFullScreen()
    }
}

/// A borderless, non-activating panel that floats over every Space —
/// including other apps' full-screen Spaces — without stealing focus.
final class FloatingPanel: NSPanel {
    init(content: some View) {
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 640, height: 180),
            styleMask: [.borderless, .nonactivatingPanel, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        isFloatingPanel = true
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isMovableByWindowBackground = true
        hidesOnDeactivate = false
        backgroundColor = .clear
        isOpaque = false
        hasShadow = false
        maxSize = NSSize(width: 1600, height: 600)

        let host = NSHostingView(rootView: content)
        host.sizingOptions = []
        contentView = host

        if !setFrameUsingName("LyricalFloatingPanel"), let screen = NSScreen.main {
            let visible = screen.visibleFrame
            setFrameOrigin(NSPoint(x: visible.midX - frame.width / 2, y: visible.minY + 72))
        }
        setFrameAutosaveName("LyricalFloatingPanel")
    }

    func applyMinimumSize(_ minimum: NSSize) {
        minSize = minimum
        contentMinSize = minimum
        guard frame.width < minimum.width || frame.height < minimum.height else { return }
        // Grow downward from the top edge so the panel doesn't jump off its spot.
        var grown = frame
        grown.size.width = max(frame.width, minimum.width)
        grown.size.height = max(frame.height, minimum.height)
        grown.origin.y = frame.maxY - grown.height
        setFrame(grown, display: true, animate: isVisible)
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

final class FullScreenWindow: NSWindow {
    init(screen: NSScreen, content: some View) {
        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        setFrame(screen.frame, display: false)
        level = .statusBar
        collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        backgroundColor = .black
        isOpaque = true
        isReleasedWhenClosed = false
        contentView = NSHostingView(rootView: content)
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}
