import SwiftUI

/// Layout constants shared by the floating view and its window, so the panel's
/// minimum size always fits the lyric rows.
enum FloatingMetrics {
    static let headerHeight: CGFloat = 16
    static let headerSpacing: CGFloat = 6
    static let verticalPadding: CGFloat = 16
    static let outerPadding: CGFloat = 8
    static let artworkSize: CGFloat = 62

    static func rowHeight(fontScale: Double) -> CGFloat { (32 * fontScale).rounded() }

    static func minimumSize(fontScale: Double, rows: Int, showArtwork: Bool) -> NSSize {
        let content = headerHeight + headerSpacing + rowHeight(fontScale: fontScale) * CGFloat(rows)
        let height = max(content, showArtwork ? artworkSize : 0) + (verticalPadding + outerPadding) * 2
        return NSSize(width: showArtwork ? 460 : 380, height: height.rounded(.up))
    }
}

struct FloatingLyricsView: View {
    @Environment(NowPlayingModel.self) private var model
    @Environment(WindowManager.self) private var windows

    @AppStorage(Prefs.syncOffset) private var syncOffset = 0.0
    @AppStorage(Prefs.panelGlass) private var panelGlass = PanelGlass.regular.rawValue
    @AppStorage(Prefs.showArtwork) private var showArtwork = true

    @State private var hovering = false

    var body: some View {
        HStack(spacing: 16) {
            if showArtwork {
                ArtworkView(image: model.artwork, cornerRadius: 12, palette: model.palette)
                    .frame(width: FloatingMetrics.artworkSize, height: FloatingMetrics.artworkSize)
                    .shadow(color: .black.opacity(0.25), radius: 8, y: 4)
                    .id(model.track?.id)
                    .transition(.blurReplace)
            }

            TimelineView(.animation(minimumInterval: 1 / 30, paused: !model.isPlaying)) { context in
                FloatingLyricsContent(time: model.time(at: context.date) + syncOffset)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.leading, showArtwork ? 18 : 26)
        .padding(.trailing, 26)
        .padding(.vertical, FloatingMetrics.verticalPadding)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .lyricalGlass(PanelGlass(rawValue: panelGlass) ?? .regular, accent: model.accent,
                      in: .rect(cornerRadius: 30, style: .continuous))
        .overlay(alignment: .topTrailing) {
            HoverControls()
                .padding(10)
                .opacity(hovering ? 1 : 0)
                .scaleEffect(hovering ? 1 : 0.9, anchor: .topTrailing)
        }
        .onHover { inside in
            withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) { hovering = inside }
        }
        .padding(FloatingMetrics.outerPadding)
        .animation(.smooth, value: model.track?.id)
    }
}

private struct FloatingLyricsContent: View {
    @Environment(NowPlayingModel.self) private var model
    @AppStorage(Prefs.fontScale) private var fontScale = 1.0
    @AppStorage(Prefs.showNextLine) private var showSurroundingLines = true
    @AppStorage(Prefs.karaokeSweep) private var karaokeSweep = true

    let time: TimeInterval

    private var rowHeight: CGFloat { FloatingMetrics.rowHeight(fontScale: fontScale) }
    private var visibleRows: Int { showSurroundingLines ? 3 : 1 }

    var body: some View {
        VStack(alignment: .leading, spacing: FloatingMetrics.headerSpacing) {
            header
                .frame(height: FloatingMetrics.headerHeight)
            Group {
                switch model.lyrics {
                case .synced(let lines):
                    LyricRoller(
                        lines: lines, time: time, rowHeight: rowHeight,
                        fontScale: fontScale, karaokeSweep: karaokeSweep)
                case .loading:
                    message("Finding lyrics…", symbol: "text.magnifyingglass")
                case .plain:
                    message("Lyrics for this song aren't time-synced", symbol: "text.alignleft")
                case .instrumental:
                    InstrumentalDots(size: 9 * fontScale, color: .primary)
                case .unavailable:
                    message("No lyrics found for this song", symbol: "quote.bubble")
                case .idle:
                    message("Play something to see lyrics", symbol: "music.note")
                }
            }
            .frame(maxWidth: .infinity, minHeight: rowHeight * CGFloat(visibleRows),
                   maxHeight: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var header: some View {
        if let track = model.track {
            HStack(spacing: 6) {
                if let source = model.source {
                    Image(systemName: source.symbol).font(.system(size: 9, weight: .bold))
                }
                Text(track.artist.isEmpty ? track.title : "\(track.title) — \(track.artist)")
                    .lineLimit(1)
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.secondary)
        }
    }

    private func message(_ text: String, symbol: String) -> some View {
        Label(text, systemImage: symbol)
            .font(.system(size: 17 * fontScale, weight: .semibold))
            .foregroundStyle(.secondary)
            .lineLimit(1)
            .minimumScaleFactor(0.7)
    }
}

/// Lyrics on a vertical roller: the current line sits in the middle at full
/// size, neighbours are smaller and dimmer, and the roller springs to the next
/// line when it begins.
private struct LyricRoller: View {
    let lines: [LyricLine]
    let time: TimeInterval
    let rowHeight: CGFloat
    let fontScale: Double
    let karaokeSweep: Bool

    /// Row -1 is a virtual intro row shown before the first line starts.
    private static let introRow = -1
    private static let inactiveScale: CGFloat = 0.7

    var body: some View {
        let active = lines.activeIndex(at: time)
        let focus = active ?? Self.introRow
        let window = max(Self.introRow, focus - 3)...min(lines.count - 1, focus + 3)
        let font = Font.system(size: 22 * fontScale, weight: .bold)

        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                ForEach(Array(window), id: \.self) { row in
                    let isFocus = row == focus
                    content(for: row, focus: focus, font: font)
                        .lineLimit(1)
                        .minimumScaleFactor(0.5)
                        .frame(width: geo.size.width / (isFocus ? 1 : Self.inactiveScale),
                               height: rowHeight, alignment: .leading)
                        .scaleEffect(isFocus ? 1 : Self.inactiveScale, anchor: .leading)
                        .opacity(row < focus ? 0.45 : 1)
                        .blur(radius: isFocus ? 0 : 0.4)
                        .offset(y: CGFloat(row) * rowHeight)
                }
            }
            .frame(width: geo.size.width, height: geo.size.height, alignment: .topLeading)
            .offset(y: (geo.size.height - rowHeight) / 2 - CGFloat(focus) * rowHeight)
            .animation(.spring(response: 0.5, dampingFraction: 0.86), value: focus)
        }
        .clipped()
        .mask(
            LinearGradient(stops: [
                .init(color: .clear, location: 0),
                .init(color: .black, location: 0.18),
                .init(color: .black, location: 0.82),
                .init(color: .clear, location: 1),
            ], startPoint: .top, endPoint: .bottom)
        )
    }

    @ViewBuilder
    private func content(for row: Int, focus: Int, font: Font) -> some View {
        if row == Self.introRow || lines[row].isGap {
            InstrumentalDots(size: 8 * fontScale, color: .primary)
                .opacity(row == focus ? 1 : 0.5)
        } else {
            // Future lines show unlit, past lines fully lit, so a line flows
            // seamlessly into and out of the sweep as it passes the middle.
            SweepText(text: lines[row].text, progress: progress(for: row, focus: focus), font: font)
        }
    }

    private func progress(for row: Int, focus: Int) -> Double {
        guard karaokeSweep else { return 1 }
        if row < focus { return 1 }
        if row > focus { return 0 }
        return lines.sweepProgress(at: row, time: time)
    }
}

private struct HoverControls: View {
    @Environment(NowPlayingModel.self) private var model
    @Environment(WindowManager.self) private var windows

    var body: some View {
        AdaptiveGlassContainer(spacing: 6) {
            HStack(spacing: 6) {
                if model.canControl {
                    control(model.isPlaying ? "pause.fill" : "play.fill", help: "Play/Pause") { model.togglePlayPause() }
                    control("forward.fill", help: "Next") { model.next() }
                }
                control("arrow.up.left.and.arrow.down.right", help: "Full Screen Lyrics") { windows.showFullScreen() }
                control("xmark", help: "Hide Floating Lyrics") { windows.setFloating(false) }
            }
        }
    }

    private func control(_ symbol: String, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .bold))
                .contentTransition(.symbolEffect(.replace))
                .frame(width: 24, height: 24)
        }
        .buttonStyle(.plain)
        .adaptiveGlass(.regular.interactive(), in: .circle)
        .help(help)
    }
}
