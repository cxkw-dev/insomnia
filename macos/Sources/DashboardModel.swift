import Foundation
import IOKit.pwr_mgt

final class DisplayAssertion {
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

@MainActor
final class DashboardModel: ObservableObject {
    /// How often the snapshot refreshes while the popover is closed.
    static let backgroundInterval: TimeInterval = 60
    /// Opening the popover refreshes data older than this.
    static let staleAfter: TimeInterval = 10

    @Published private(set) var content = DashboardContent()
    @Published private(set) var hasCards = false
    @Published private(set) var isAwake = false
    @Published private(set) var isLoading = false
    @Published private(set) var refreshError: String?
    @Published private(set) var assertionError: String?
    @Published private(set) var refreshedAt: Date?
    /// The popover's account editor is open.
    @Published var isEditingAccounts = false
    /// Accounts turned off in accounts.conf; listed only by the editor.
    @Published private(set) var hiddenAccounts: [AccountVisibility.Entry] = []

    private let assertion = DisplayAssertion()
    private var refreshTimer: Timer?
    /// A refresh was asked for while one was running (e.g. right after a
    /// visibility change); run it as soon as the current one lands.
    private var refreshQueued = false

    init() {
        if ProcessInfo.processInfo.environment["INSOMNIA_START_AWAKE"] == "1" {
            setAwake(true)
        }
        refresh()
        refreshTimer = Timer.scheduledTimer(withTimeInterval: Self.backgroundInterval, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        }
    }

    func setAwake(_ enabled: Bool) {
        do {
            try assertion.setEnabled(enabled)
            assertionError = nil
        } catch {
            assertionError = error.localizedDescription
        }
        isAwake = assertion.isActive
    }

    func refreshIfStale() {
        if let refreshedAt, Date().timeIntervalSince(refreshedAt) < Self.staleAfter { return }
        refresh()
    }

    /// Turn an account on or off in accounts.conf, update the editor at once,
    /// and refresh so the change reaches the rest of the popover.
    func setHidden(_ hidden: Bool, service: String, name: String) {
        do {
            try AccountVisibility.setHidden(hidden, service: service, name: name)
            refreshError = nil
        } catch {
            refreshError = "Could not update \(AccountVisibility.url.path): \(error.localizedDescription)"
            return
        }
        hiddenAccounts = AccountVisibility.hidden()
        if hidden {
            content.accounts.removeAll { $0.provider.service == service && $0.name == name }
        }
        refresh()
    }

    func refresh() {
        hiddenAccounts = AccountVisibility.hidden()
        guard !isLoading else {
            refreshQueued = true
            return
        }
        isLoading = true
        Task {
            defer {
                isLoading = false
                if refreshQueued {
                    refreshQueued = false
                    refresh()
                }
            }
            do {
                let snapshot = try await SnapshotRunner.load()
                content = parseDashboard(snapshot.cards, groups: snapshot.accounts ?? AccountGroups())
                hasCards = !snapshot.cards.isEmpty
                refreshedAt = Date()
                refreshError = nil
            } catch {
                refreshError = error.localizedDescription
            }
        }
    }
}
