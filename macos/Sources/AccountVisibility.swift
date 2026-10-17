import Foundation

/// Account visibility lives in ai-usage's `accounts.conf`: a line
/// `<service> <name> hidden` turns an account off. The script stops fetching
/// it and both apps stop showing it, so this file is the single source of
/// truth; the menu bar only edits it. Comments and other lines are kept.
enum AccountVisibility {
    struct Entry: Hashable, Identifiable {
        let service: String
        let name: String

        var id: String { "\(service) \(name)" }
        var provider: AIProvider? { AIProvider(service: service) }
    }

    static var url: URL {
        let environment = ProcessInfo.processInfo.environment
        let base = environment["XDG_CONFIG_HOME"].map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".config")
        return base.appendingPathComponent("insomnia/ai-usage/accounts.conf")
    }

    static func hidden() -> [Entry] {
        lines().compactMap { line in
            let w = words(line)
            return w.count >= 3 && w[2] == "hidden" ? Entry(service: w[0], name: w[1]) : nil
        }
    }

    /// Hide or show one account. Showing keeps the line without `hidden`, so
    /// the account stays listed (as "not logged in" if it has no login).
    static func setHidden(_ hidden: Bool, service: String, name: String) throws {
        var lines = lines()
        let line = hidden ? "\(service) \(name) hidden" : "\(service) \(name)"
        if let index = lines.firstIndex(where: { words($0).prefix(2) == [service, name] }) {
            lines[index] = line
        } else if hidden {
            if lines.last == "" { lines.removeLast() }
            lines.append(line)
        } else {
            return
        }
        try write(lines)
    }

    private static func lines() -> [String] {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        return text.components(separatedBy: "\n")
    }

    private static func words(_ line: String) -> [String] {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.hasPrefix("#") { return [] }
        return trimmed.split(whereSeparator: \.isWhitespace).map(String.init)
    }

    private static func write(_ lines: [String]) throws {
        var text = lines.joined(separator: "\n")
        if !text.hasSuffix("\n") { text += "\n" }
        let manager = FileManager.default
        try manager.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        try text.write(to: url, atomically: true, encoding: .utf8)
        try manager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }
}
