import AppKit
import SwiftUI

enum AIProvider {
    case claude, codex, copilot

    /// Recognizes ai-usage.sh section headers (`✻ claude`, `◎ openai codex`,
    /// `⧉ copilot`) by their last word only, so an unrelated header such as
    /// `claude jobs` stays an ordinary section.
    init?(section: String) {
        let words = section.drop { !$0.isLetter }.lowercased().split(whereSeparator: \.isWhitespace)
        switch words.last {
        case "claude": self = .claude
        case "codex": self = .codex
        case "copilot": self = .copilot
        default: return nil
        }
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

enum AccountCategory: Hashable, CaseIterable {
    case business, personal

    var title: String { self == .business ? "Business AI" : "Personal" }
}

/// The config's `[accounts]` table: name substrings that pin an account to a group.
struct AccountGroups: Decodable, Sendable {
    var personal: [String] = []
    var business: [String] = []

    func category(for name: String) -> AccountCategory? {
        let identity = name.lowercased()
        func claims(_ patterns: [String]) -> Bool {
            patterns.contains { identity.contains($0.lowercased()) }
        }
        if claims(personal) { return .personal }
        if claims(business) { return .business }
        return nil
    }
}

struct AIAccount: Identifiable {
    struct Meter {
        let name: String
        let percent: UInt8
        let detail: String?
    }

    let id: String
    let provider: AIProvider
    let category: AccountCategory
    let name: String
    let detail: String?
    let health: Health?
    var meters: [Meter] = []

    init(id: String, provider: AIProvider, row: CardRow, groups: AccountGroups) {
        self.id = id
        self.provider = provider
        name = row.name.replacingOccurrences(
            of: "^[▸›\\s]+", with: "", options: .regularExpression
        )
        detail = row.detail
        health = row.health
        let planWords = Set((row.detail ?? "").lowercased().components(
            separatedBy: CharacterSet.alphanumerics.inverted
        ))
        category = groups.category(for: name) ??
            (["org", "business", "enterprise", "ent", "biz", "team"]
                .contains { planWords.contains($0) } ? .business : .personal)
    }
}

/// A card's rows that are not part of an AI provider section.
struct DashboardCard: Identifiable {
    let id: Int
    let title: String
    let error: String?
    let rows: [CardRow]
}

struct DashboardContent {
    var accounts: [AIAccount] = []
    var cards: [DashboardCard] = []

    func accounts(in category: AccountCategory) -> [AIAccount] {
        accounts.filter { $0.category == category }
    }
}

/// Splits snapshot cards into AI accounts (one per health row inside a
/// provider section, with the meters that follow it) and everything else.
/// Rows printed before a card's first section, such as ai-usage.sh's
/// "nearest limit" rollup, stay with the card.
func parseDashboard(_ snapshotCards: [SnapshotCard], groups: AccountGroups) -> DashboardContent {
    var content = DashboardContent()
    for (cardIndex, card) in snapshotCards.enumerated() {
        let hasAISection = card.rows.contains { $0.section && AIProvider(section: $0.name) != nil }
        var provider: AIProvider?
        var account: AIAccount?
        var seenSection = false
        var rows: [CardRow] = []
        for (rowIndex, row) in card.rows.enumerated() {
            if row.section {
                if let account { content.accounts.append(account) }
                account = nil
                provider = AIProvider(section: row.name)
                seenSection = true
            }
            if let provider {
                if row.section { continue }
                if row.health != nil {
                    if let account { content.accounts.append(account) }
                    account = AIAccount(
                        id: "\(cardIndex)-\(rowIndex)", provider: provider, row: row, groups: groups
                    )
                } else if let percent = row.percent {
                    account?.meters.append(.init(name: row.name, percent: percent, detail: row.detail))
                }
            } else if !hasAISection || seenSection || row.health != nil || row.percent != nil {
                rows.append(row)
            }
        }
        if let account { content.accounts.append(account) }
        if !rows.isEmpty || card.error != nil {
            content.cards.append(DashboardCard(id: cardIndex, title: card.title, error: card.error, rows: rows))
        }
    }
    return content
}
