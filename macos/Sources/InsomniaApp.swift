import AppKit
import CoreText
import Foundation
import IOKit.pwr_mgt
import SwiftUI

private enum Appearance {
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

private enum Typography {
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

private struct Snapshot: Decodable, Sendable {
    let cards: [UsageCard]
}

private struct UsageCard: Decodable, Sendable {
    let title: String
    let rows: [UsageRow]
    let error: String?
}

private struct UsageRow: Decodable, Sendable {
    let health: String?
    let name: String
    let detail: String?
    let accent: [UInt8]?
    let section: Bool
    let percent: UInt8?

    var color: Color {
        guard let accent, accent.count == 3 else { return .purple }
        return Color(
            .sRGB,
            red: Double(accent[0]) / 255,
            green: Double(accent[1]) / 255,
            blue: Double(accent[2]) / 255,
            opacity: 1
        )
    }

    var healthColor: Color {
        switch health {
        case "good": return Appearance.success
        case "warn": return Appearance.warning
        case "bad": return Appearance.danger
        default: return Appearance.quiet
        }
    }
}

private final class DisplayAssertion {
    private var id: IOPMAssertionID?

    var isActive: Bool { id != nil }

    func setEnabled(_ enabled: Bool) throws {
        if enabled && id == nil {
            var newID: IOPMAssertionID = 0
            let result = IOPMAssertionCreateWithName(
                kIOPMAssertionTypePreventUserIdleDisplaySleep as CFString,
                IOPMAssertionLevel(kIOPMAssertionLevelOn),
                "Insomnia: keep display awake" as CFString,
                &newID
            )
            guard result == kIOReturnSuccess else {
                throw AppError.message("Could not keep the display awake (\(result)).")
            }
            id = newID
        } else if !enabled, let id {
            IOPMAssertionRelease(id)
            self.id = nil
        }
    }

    deinit {
        if let id { IOPMAssertionRelease(id) }
    }
}

private enum AppError: LocalizedError {
    case message(String)

    var errorDescription: String? {
        if case let .message(value) = self { return value }
        return nil
    }
}

private enum SnapshotResult: Sendable {
    case success(Snapshot)
    case failure(String)
}

private enum SnapshotRunner {
    static func load() throws -> Snapshot {
        guard let helper = Bundle.main.path(forAuxiliaryExecutable: "insomnia-snapshot") else {
            throw AppError.message("The usage helper is missing from the app bundle.")
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: helper)
        var environment = ProcessInfo.processInfo.environment
        let home = environment["HOME"] ?? NSHomeDirectory()
        environment["PATH"] = [
            "\(home)/.local/bin", "\(home)/.cargo/bin", "/opt/homebrew/bin",
            "/usr/local/bin", "/usr/bin", "/bin", environment["PATH"] ?? ""
        ].joined(separator: ":")
        process.environment = environment
        let output = Pipe()
        let errors = Pipe()
        process.standardOutput = output
        process.standardError = errors
        try process.run()
        DispatchQueue.global().asyncAfter(deadline: .now() + 20) {
            if process.isRunning { process.terminate() }
        }
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let message = String(data: errors.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw AppError.message(message?.isEmpty == false ? message! : "Usage refresh failed or timed out.")
        }
        return try JSONDecoder().decode(Snapshot.self, from: data)
    }
}

@MainActor
private final class DashboardModel: ObservableObject {
    @Published private(set) var cards: [UsageCard] = []
    @Published private(set) var isAwake = false
    @Published private(set) var loading = false
    @Published private(set) var message: String?
    @Published private(set) var refreshedAt: Date?

    private let assertion = DisplayAssertion()
    private var refreshTimer: Timer?

    init() {
        if ProcessInfo.processInfo.environment["INSOMNIA_START_AWAKE"] == "1" {
            setAwake(true)
        }
        refresh()
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        }
    }

    var hasCards: Bool { !cards.isEmpty }

    func setAwake(_ enabled: Bool) {
        do {
            try assertion.setEnabled(enabled)
            isAwake = assertion.isActive
            message = nil
        } catch {
            isAwake = assertion.isActive
            message = error.localizedDescription
        }
    }

    func refresh() {
        guard !loading else { return }
        loading = true
        Task {
            let result = await Task.detached(priority: .utility) { () -> SnapshotResult in
                do { return .success(try SnapshotRunner.load()) }
                catch { return .failure(error.localizedDescription) }
            }.value
            loading = false
            switch result {
            case let .success(snapshot):
                cards = snapshot.cards
                refreshedAt = Date()
                message = nil
            case let .failure(error):
                message = error
            }
        }
    }
}

private struct UsageMeter: View {
    let percent: UInt8
    let color: Color

    var body: some View {
        Canvas { context, size in
            let track = Path(CGRect(x: 0, y: 0, width: size.width, height: size.height))
            let used = Path(CGRect(x: 0, y: 0, width: size.width * CGFloat(percent) / 100, height: size.height))
            context.fill(track, with: .color(Appearance.track))
            context.fill(used, with: .color(color))
        }
        .frame(height: 2)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(percent) percent used")
    }
}

private enum StatusIcon {
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
private enum ProviderIcon {
    private static func image(bundleID: String, resource: String) -> NSImage? {
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else {
            return nil
        }
        return NSImage(contentsOf: app.appendingPathComponent("Contents/Resources/\(resource)"))
    }

    static let claude = image(
        bundleID: "com.anthropic.claudefordesktop",
        resource: "TrayIconTemplate-Dark@3x.png"
    )
    private static func bundledImage(_ name: String, extension ext: String) -> NSImage? {
        guard let url = Bundle.main.url(forResource: name, withExtension: ext, subdirectory: "Icons") else {
            return nil
        }
        return NSImage(contentsOf: url)
    }

    static let codex = bundledImage("codex", extension: "png")
    static let copilot: NSImage? = {
        let image = bundledImage("copilot", extension: "svg")
        image?.isTemplate = true
        return image
    }()
}

private enum AIProvider {
    case claude, codex, copilot

    init?(section: String) {
        let name = section.lowercased()
        if name.contains("claude") { self = .claude }
        else if name.contains("codex") { self = .codex }
        else if name.contains("copilot") { self = .copilot }
        else { return nil }
    }

    var label: String {
        switch self {
        case .claude: "Claude"
        case .codex: "Codex"
        case .copilot: "Copilot"
        }
    }

    var color: Color {
        switch self {
        case .claude: Appearance.color(0xd97757)
        case .codex: Appearance.color(0x6978f5)
        case .copilot: Appearance.color(0x8534f3)
        }
    }

    var iconSize: CGFloat {
        switch self {
        case .claude: 23 // The tray image has transparent padding around the mark.
        case .codex: 16
        case .copilot: 17
        }
    }

    @MainActor var icon: NSImage? {
        switch self {
        case .claude: ProviderIcon.claude
        case .codex: ProviderIcon.codex
        case .copilot: ProviderIcon.copilot
        }
    }
}

private struct ProviderMark: View {
    let provider: AIProvider

    var body: some View {
        Group {
            if let icon = provider.icon {
                Image(nsImage: icon)
                    .resizable()
                    .renderingMode(.template)
                    .interpolation(.high)
                    .scaledToFit()
                    .frame(width: provider.iconSize, height: provider.iconSize)
                    .foregroundStyle(provider.color)
            } else {
                Circle()
                    .fill(provider.color)
                    .frame(width: 7, height: 7)
            }
        }
        .frame(width: 18, height: 18)
        .accessibilityHidden(true)
    }
}

private enum AccountCategory: Hashable {
    case business, personal

    var title: String { self == .business ? "Business AI" : "Personal" }
}

private struct CrescentGlyph: Shape {
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

private struct InsomniaWordmark: View {
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

private struct AIAccount: Identifiable {
    let id: String
    let provider: AIProvider
    let category: AccountCategory
    let name: String
    let detail: String?
    var meters: [UsageRow] = []

    init(id: String, provider: AIProvider, row: UsageRow) {
        self.id = id
        self.provider = provider
        name = row.name.replacingOccurrences(
            of: "^[▸›\\s]+", with: "", options: .regularExpression
        )
        detail = row.detail
        let identity = name.lowercased()
        let planWords = Set((row.detail ?? "").lowercased().components(
            separatedBy: CharacterSet.alphanumerics.inverted
        ))
        category = identity.contains("cxkw.dev") ? .personal :
            (["org", "business", "enterprise", "ent", "biz", "team"]
                .contains { planWords.contains($0) } ? .business : .personal)
    }
}

private struct DashboardView: View {
    @ObservedObject var model: DashboardModel

    private var aiAccounts: [AIAccount] {
        var accounts: [AIAccount] = []
        for (cardIndex, card) in model.cards.enumerated() {
            var provider: AIProvider?
            var account: AIAccount?
            for (rowIndex, row) in card.rows.enumerated() {
                if row.section {
                    if let account { accounts.append(account) }
                    account = nil
                    provider = AIProvider(section: row.name)
                } else if let provider, row.health != nil,
                          !row.name.lowercased().contains("nearest limit") {
                    if let account { accounts.append(account) }
                    account = AIAccount(id: "\(cardIndex)-\(rowIndex)", provider: provider, row: row)
                } else if row.percent != nil {
                    account?.meters.append(row)
                }
            }
            if let account { accounts.append(account) }
        }
        return accounts
    }

    private func otherRows(in card: UsageCard) -> [UsageRow] {
        let hasAISection = card.rows.contains { $0.section && AIProvider(section: $0.name) != nil }
        guard hasAISection else { return card.rows }

        var rows: [UsageRow] = []
        var inAISection = false
        var seenSection = false
        for row in card.rows {
            if row.section {
                seenSection = true
                inAISection = AIProvider(section: row.name) != nil
            }
            if !inAISection && (seenSection || row.health != nil || row.percent != nil) {
                rows.append(row)
            }
        }
        return rows
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .center) {
                InsomniaWordmark()
                Spacer()
                Toggle("Keep display awake", isOn: Binding(
                    get: { model.isAwake },
                    set: { model.setAwake($0) }
                ))
                .labelsHidden()
                .toggleStyle(.switch)
                .tint(Appearance.awake)
                .controlSize(.small)
                .help(model.isAwake ? "Display awake: turn off" : "Keep display awake")
            }
            .padding(.bottom, 11)

            VStack(alignment: .leading, spacing: 18) {
                if !model.hasCards && model.message == nil {
                    Text("Add an AI usage card to ~/.config/insomnia/config.toml to see usage here.")
                        .font(Typography.sans(13))
                        .foregroundStyle(Appearance.muted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                ForEach([AccountCategory.business, .personal], id: \.self) { category in
                    let accounts = aiAccounts.filter { $0.category == category }
                    if !accounts.isEmpty {
                        accountSection(category, accounts: accounts)
                    }
                }
                ForEach(Array(model.cards.enumerated()), id: \.offset) { _, card in
                    let rows = otherRows(in: card)
                    if !rows.isEmpty || card.error != nil {
                        VStack(alignment: .leading, spacing: 5) {
                            Text(card.title)
                                .font(Typography.sans(11, weight: .medium))
                                .foregroundStyle(Appearance.quiet)
                            if let error = card.error {
                                Text(error).foregroundStyle(Appearance.warning).font(Typography.sans(12))
                            }
                            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                                usageRow(row)
                            }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            if let message = model.message {
                Text(message)
                    .font(Typography.sans(12))
                    .foregroundStyle(Appearance.warning)
                    .lineLimit(3)
                    .padding(.top, 10)
            }

            Rectangle().fill(Appearance.border).frame(height: 1).padding(.top, 11)
            HStack {
                Button {
                    model.refresh()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.loading)
                Spacer()
                if model.loading { ProgressView().controlSize(.mini) }
                if let refreshedAt = model.refreshedAt {
                    Text(refreshedAt, style: .time)
                        .font(Typography.mono(10))
                        .foregroundStyle(Appearance.quiet)
                }
                Button("Quit") { NSApplication.shared.terminate(nil) }
            }
            .buttonStyle(.plain)
            .font(Typography.sans(11, weight: .medium))
            .foregroundStyle(Appearance.muted)
            .padding(.top, 9)
        }
        .foregroundStyle(Appearance.text)
        .padding(18)
        .frame(width: 390)
        .background(Appearance.background)
        .preferredColorScheme(.dark)
        .onAppear { if model.refreshedAt == nil { model.refresh() } }
    }

    private func accountSection(_ category: AccountCategory, accounts: [AIAccount]) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text(category.title)
                    .font(Typography.sans(13, weight: .semibold))
                Spacer()
                Text("\(accounts.count) \(accounts.count == 1 ? "account" : "accounts")")
                    .font(Typography.sans(10))
                    .foregroundStyle(Appearance.quiet)
            }
            ForEach(accounts) { account in
                accountView(account)
            }
        }
        .padding(.top, 12)
        .overlay(alignment: .top) {
            Rectangle().fill(Appearance.border).frame(height: 1)
        }
    }

    private func accountView(_ account: AIAccount) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 8) {
                ProviderMark(provider: account.provider)
                Text(account.provider.label)
                    .font(Typography.sans(12, weight: .medium))
                Spacer(minLength: 8)
                Text(account.name)
                    .font(Typography.sans(11))
                    .foregroundStyle(Appearance.muted)
                    .lineLimit(1)
            }
            if let detail = account.detail {
                Text(detail)
                    .font(Typography.sans(10))
                    .foregroundStyle(Appearance.quiet)
                    .lineLimit(1)
                    .padding(.leading, 26)
            }
            ForEach(Array(account.meters.enumerated()), id: \.offset) { _, meter in
                if let percent = meter.percent {
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(meter.name)
                                .foregroundStyle(Appearance.muted)
                            Spacer()
                            Text("\(percent)%")
                                .font(Typography.mono(11, weight: .semibold))
                                .foregroundStyle(percent >= 95 ? Appearance.danger : Appearance.text)
                        }
                        .font(Typography.sans(11))
                        UsageMeter(percent: percent, color: account.provider.color)
                        if let detail = meter.detail {
                            Text(detail)
                                .font(Typography.sans(10))
                                .foregroundStyle(Appearance.quiet)
                                .lineLimit(1)
                        }
                    }
                    .padding(.leading, 26)
                }
            }
        }
    }

    @ViewBuilder
    private func usageRow(_ row: UsageRow) -> some View {
        if row.section {
            HStack(spacing: 9) {
                if let provider = AIProvider(section: row.name) {
                    ProviderMark(provider: provider)
                } else {
                    Circle().fill(Appearance.text).frame(width: 6, height: 6)
                        .frame(width: 18, height: 18)
                }
                Text(AIProvider(section: row.name)?.label ?? row.name)
                    .font(Typography.sans(12, weight: .medium))
                    .foregroundStyle(Appearance.text)
                Spacer()
            }
            .padding(.top, 9)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .top) {
                Rectangle().fill(Appearance.border).frame(height: 1)
            }
        } else if let percent = row.percent {
            let meterColor: Color = percent >= 95 ? Appearance.danger : Appearance.text
            VStack(alignment: .leading, spacing: 3) {
                HStack {
                    Text(row.name)
                        .foregroundStyle(Appearance.muted)
                    Spacer()
                    Text("\(percent)%")
                        .foregroundStyle(meterColor)
                        .font(Typography.mono(11, weight: .semibold))
                }
                .font(Typography.sans(11, weight: .medium))
                UsageMeter(percent: percent, color: Appearance.muted.opacity(0.72))
                if let detail = row.detail {
                    Text(detail)
                        .font(Typography.sans(10))
                        .foregroundStyle(Appearance.quiet)
                        .lineLimit(1)
                }
            }
            .padding(.leading, 24)
        } else if let health = row.health {
            HStack(alignment: .firstTextBaseline, spacing: 7) {
                Text(row.name)
                    .font(Typography.sans(11, weight: .medium))
                Spacer(minLength: 6)
                if let detail = row.detail {
                    Text(detail)
                        .font(Typography.sans(10))
                        .foregroundStyle(health == "bad" ? Appearance.text : Appearance.muted)
                        .multilineTextAlignment(.trailing)
                        .lineLimit(1)
                }
            }
            .padding(.leading, 24)
        } else {
            Text(row.name)
                .font(Typography.sans(11))
                .foregroundStyle(Appearance.muted)
        }
    }
}

@main
private struct InsomniaApp: App {
    @StateObject private var model = DashboardModel()

    init() {
        Typography.registerBundledFonts()
    }

    var body: some Scene {
#if DESIGN_PREVIEW
        WindowGroup("Insomnia Design Preview") {
            DashboardView(model: model)
        }
#else
        MenuBarExtra {
            DashboardView(model: model)
        } label: {
            if model.isAwake {
                Image(nsImage: StatusIcon.awake)
                    .resizable()
                    .frame(width: 18, height: 18)
                    .accessibilityLabel("Insomnia: display awake")
            } else {
                Image(systemName: "moon.fill")
                    .font(.system(size: 15, weight: .medium))
                    .accessibilityLabel("Insomnia: display sleep normal")
            }
        }
        .menuBarExtraStyle(.window)
#endif
    }
}
