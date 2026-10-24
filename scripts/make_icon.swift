// Renders the app icon: a glossy rose-to-violet squircle with a lyrics glyph.
import AppKit

let out = CommandLine.arguments[1]
let size: CGFloat = 1024
let image = NSImage(size: NSSize(width: size, height: size))
image.lockFocus()
let inset: CGFloat = 100
let rect = NSRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
let path = NSBezierPath(roundedRect: rect, xRadius: 185, yRadius: 185)

NSGraphicsContext.current?.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(0.3)
shadow.shadowBlurRadius = 30
shadow.shadowOffset = NSSize(width: 0, height: -12)
shadow.set()
NSColor.black.setFill()
path.fill()
NSGraphicsContext.current?.restoreGraphicsState()

NSGradient(colors: [
    NSColor(red: 1.00, green: 0.36, blue: 0.45, alpha: 1),
    NSColor(red: 0.93, green: 0.18, blue: 0.42, alpha: 1),
    NSColor(red: 0.52, green: 0.20, blue: 0.86, alpha: 1),
])!.draw(in: path, angle: -60)

// Soft glass highlight across the top.
NSGraphicsContext.current?.saveGraphicsState()
path.addClip()
NSGradient(colors: [NSColor.white.withAlphaComponent(0.35), NSColor.white.withAlphaComponent(0)])!
    .draw(in: NSRect(x: rect.minX, y: rect.midY, width: rect.width, height: rect.height / 2), angle: -90)
NSGraphicsContext.current?.restoreGraphicsState()

let config = NSImage.SymbolConfiguration(pointSize: 440, weight: .semibold)
if let symbol = NSImage(systemSymbolName: "quote.bubble.fill", accessibilityDescription: nil)?.withSymbolConfiguration(config) {
    let tinted = NSImage(size: symbol.size)
    tinted.lockFocus()
    symbol.draw(at: .zero, from: .zero, operation: .sourceOver, fraction: 1)
    NSColor.white.set()
    NSRect(origin: .zero, size: symbol.size).fill(using: .sourceAtop)
    tinted.unlockFocus()
    let origin = NSPoint(x: (size - symbol.size.width) / 2, y: (size - symbol.size.height) / 2 - 10)
    tinted.draw(at: origin, from: .zero, operation: .sourceOver, fraction: 0.97)
}
image.unlockFocus()

let rep = NSBitmapImageRep(data: image.tiffRepresentation!)!
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
