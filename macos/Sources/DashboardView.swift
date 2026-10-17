import AppKit
import SwiftUI

struct DashboardView: View {
    @ObservedObject var model: DashboardModel

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
                if !model.hasCards && model.refreshError == nil {
                    Text("Add an AI usage card to ~/.config/insomnia/config.toml to see usage here.")
                        .font(Typography.sans(13))
                        .foregroundStyle(Appearance.muted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                ForEach(AccountCategory.allCases, id: \.self) { category in
                    let accounts = model.content.accounts(in: category)
                    if !accounts.isEmpty {
                        AccountSection(category: category, accounts: accounts)
                    }
                }
                ForEach(model.content.cards) { card in
                    CardView(card: card)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            ForEach([model.assertionError, model.refreshError].compactMap { $0 }, id: \.self) { message in
                Text(message)
                    .font(Typography.sans(12))
                    .foregroundStyle(Appearance.warning)
                    .lineLimit(3)
                    .padding(.top, 10)
            }

            Hairline().padding(.top, 11)
            HStack {
                Button {
                    model.refresh()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.isLoading)
                Spacer()
                if model.isLoading { ProgressView().controlSize(.mini) }
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
        .onAppear { model.refreshIfStale() }
    }
}

private struct AccountSection: View {
    let category: AccountCategory
    let accounts: [AIAccount]

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text(category.title)
                    .font(Typography.sans(13, weight: .semibold))
                Spacer()
                Text("\(accounts.count) \(accounts.count == 1 ? "account" : "accounts")")
                    .caption()
            }
            ForEach(accounts) { account in
                AccountView(account: account)
            }
        }
        .padding(.top, 12)
        .overlay(alignment: .top) { Hairline() }
    }
}

private struct AccountView: View {
    let account: AIAccount

    /// The account name carries its health: amber when warning or stale, red
    /// when bad, dimmed when off (e.g. not logged in).
    private var nameColor: Color {
        switch account.health {
        case .warn: Appearance.warning
        case .bad: Appearance.danger
        case .off: Appearance.quiet
        case .good, nil: Appearance.muted
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 8) {
                ProviderMark(provider: account.provider)
                Text(account.provider.label)
                    .font(Typography.sans(12, weight: .medium))
                Spacer(minLength: 8)
                Text(account.name)
                    .font(Typography.sans(11))
                    .foregroundStyle(nameColor)
                    .lineLimit(1)
            }
            .opacity(account.health == .off ? 0.6 : 1)
            if let detail = account.detail {
                Text(detail)
                    .caption()
                    .padding(.leading, 26)
            }
            ForEach(Array(account.meters.enumerated()), id: \.offset) { _, meter in
                MeterRow(name: meter.name, percent: meter.percent, detail: meter.detail, tint: account.provider.color)
                    .padding(.leading, 26)
            }
        }
    }
}

private struct CardView: View {
    let card: DashboardCard

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(card.title)
                .font(Typography.sans(11, weight: .medium))
                .foregroundStyle(Appearance.quiet)
            if let error = card.error {
                Text(error).foregroundStyle(Appearance.warning).font(Typography.sans(12))
            }
            ForEach(Array(card.rows.enumerated()), id: \.offset) { _, row in
                CardRowView(row: row)
            }
        }
    }
}

private struct CardRowView: View {
    let row: CardRow

    var body: some View {
        if row.section {
            HStack(spacing: 9) {
                Circle().fill(Appearance.text).frame(width: 6, height: 6)
                    .frame(width: 18, height: 18)
                Text(row.name)
                    .font(Typography.sans(12, weight: .medium))
                    .foregroundStyle(Appearance.text)
                Spacer()
            }
            .padding(.top, 9)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .top) { Hairline() }
        } else if let percent = row.percent {
            MeterRow(name: row.name, percent: percent, detail: row.detail, tint: Appearance.muted.opacity(0.72))
                .padding(.leading, 24)
        } else if let health = row.health {
            HStack(alignment: .firstTextBaseline, spacing: 7) {
                Text(row.name)
                    .font(Typography.sans(11, weight: .medium))
                Spacer(minLength: 6)
                if let detail = row.detail {
                    Text(detail)
                        .font(Typography.sans(10))
                        .foregroundStyle(health == .bad ? Appearance.text : Appearance.muted)
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

/// A labeled usage meter: name and percent above a thin bar, detail below.
struct MeterRow: View {
    let name: String
    let percent: UInt8
    let detail: String?
    let tint: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(name)
                    .foregroundStyle(Appearance.muted)
                Spacer()
                Text("\(percent)%")
                    .font(Typography.mono(11, weight: .semibold))
                    .foregroundStyle(percent >= Threshold.danger ? Appearance.danger : Appearance.text)
            }
            .font(Typography.sans(11))
            UsageBar(percent: percent, color: tint)
            if let detail {
                Text(detail).caption()
            }
        }
    }
}

private struct UsageBar: View {
    let percent: UInt8
    let color: Color

    var body: some View {
        Canvas { context, size in
            let track = Path(CGRect(x: 0, y: 0, width: size.width, height: size.height))
            let used = Path(CGRect(x: 0, y: 0, width: size.width * CGFloat(min(percent, 100)) / 100, height: size.height))
            context.fill(track, with: .color(Appearance.track))
            context.fill(used, with: .color(color))
        }
        .frame(height: 2)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(percent) percent used")
    }
}

struct ProviderMark: View {
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
                // e.g. the Claude mark when the Claude desktop app is absent.
                Circle()
                    .fill(provider.color)
                    .frame(width: 7, height: 7)
            }
        }
        .frame(width: 18, height: 18)
        .accessibilityHidden(true)
    }
}
