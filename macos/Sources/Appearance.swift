import AppKit
import CoreText
import SwiftUI

enum Appearance {
    static func color(_ hex: UInt32) -> Color {
        Color(
            .sRGB,
            red: Double((hex >> 16) & 0xff) / 255,
            green: Double((hex >> 8) & 0xff) / 255,
            blue: Double(hex & 0xff) / 255,
            opacity: 1
        )
    }

    static let background = color(0x000000)
    static let surface = color(0x0a0a0a)
    static let border = color(0x292929)
    static let text = color(0xfafafa)
    static let muted = color(0xaaaaaa)
    static let quiet = color(0x777777)
    static let track = color(0x242424)
    static let awake = color(0xffb224)
    static let warning = color(0xf5a623)
    static let danger = color(0xff5252)
    static let success = color(0x9b9b9b)
}

/// Usage levels that change how a meter reads.
enum Threshold {
    /// Matches ai-usage.sh, which marks an account `bad` at 90% and above.
    static let danger: UInt8 = 90
}

enum Typography {
    static func registerBundledFonts() {
        for name in ["Geist", "GeistMono"] {
            guard let url = Bundle.main.url(forResource: name, withExtension: "ttf", subdirectory: "Fonts") else {
                continue
            }
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
    }

    static func sans(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        let name: String
        switch weight {
        case .bold: name = "Geist-Bold"
        case .semibold: name = "Geist-SemiBold"
        case .medium: name = "Geist-Medium"
        default: name = "Geist-Regular"
        }
        return .custom(name, size: size)
    }

    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        let name = weight == .semibold || weight == .bold ? "GeistMono-SemiBold" : "GeistMono-Regular"
        return .custom(name, size: size)
    }
}

extension View {
    /// Small, quiet, single-line secondary text.
    func caption() -> some View {
        font(Typography.sans(10))
            .foregroundStyle(Appearance.quiet)
            .lineLimit(1)
    }
}

/// The 1px divider between dashboard sections.
struct Hairline: View {
    var body: some View {
        Rectangle().fill(Appearance.border).frame(height: 1)
    }
}

enum StatusIcon {
    static let awake: NSImage = {
        let tint = NSColor(calibratedRed: 1, green: 0.7, blue: 0.14, alpha: 1)
        let symbol = NSImage(systemSymbolName: "moon.fill", accessibilityDescription: nil)!
            .withSymbolConfiguration(
                NSImage.SymbolConfiguration(pointSize: 16, weight: .medium)
                    .applying(NSImage.SymbolConfiguration(paletteColors: [tint]))
            )!
        let image = NSImage(size: NSSize(width: 22, height: 22), flipped: false) { _ in
            let glow = NSShadow()
            glow.shadowColor = tint.withAlphaComponent(0.75)
            glow.shadowBlurRadius = 6
            glow.set()
            symbol.draw(in: NSRect(x: 3, y: 3, width: 16, height: 16))
            return true
        }
        image.isTemplate = false
        return image
    }()
}

@MainActor
enum ProviderIcon {
    private static func image(bundleID: String, resource: String) -> NSImage? {
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else {
            return nil
        }
        return NSImage(contentsOf: app.appendingPathComponent("Contents/Resources/\(resource)"))
    }

    private static func bundledImage(_ name: String, extension ext: String) -> NSImage? {
        guard let url = Bundle.main.url(forResource: name, withExtension: ext, subdirectory: "Icons") else {
            return nil
        }
        return NSImage(contentsOf: url)
    }

    /// Borrowed from the Claude desktop app's tray icon rather than bundled.
    /// Without that app installed (or if it renames the resource) this is nil
    /// and `ProviderMark` falls back to a dot in the provider's color.
    static let claude = image(
        bundleID: "com.anthropic.claudefordesktop",
        resource: "TrayIconTemplate-Dark@3x.png"
    )
    static let codex = bundledImage("codex", extension: "png")
    static let copilot: NSImage? = {
        let image = bundledImage("copilot", extension: "svg")
        image?.isTemplate = true
        return image
    }()
}

struct CrescentGlyph: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.addEllipse(in: rect)
        path.addEllipse(in: CGRect(
            x: rect.minX + rect.width * 0.484,
            y: rect.minY - rect.height * 0.266,
            width: rect.width * 0.906,
            height: rect.height * 0.906
        ))
        return path
    }
}

struct InsomniaWordmark: View {
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 0) {
            Text("Ins").tracking(-2.6)
            CrescentGlyph()
                .fill(Appearance.awake, style: FillStyle(eoFill: true))
                .frame(width: 30.1, height: 30.1)
                // The cutout ellipse extends past the disc; hide that outer portion.
                .clipShape(Circle())
                .padding(.leading, 0.94)
                .padding(.trailing, 0.24)
                .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 1.18 }
            Text("mnia").tracking(-2.6)
        }
        .font(Typography.sans(47, weight: .semibold))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Insomnia")
    }
}
