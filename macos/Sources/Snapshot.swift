import Foundation

/// The JSON printed by the bundled `insomnia-snapshot` helper.
struct Snapshot: Decodable, Sendable {
    let cards: [SnapshotCard]
    let accounts: AccountGroups?
}

struct SnapshotCard: Decodable, Sendable {
    let title: String
    let rows: [CardRow]
    let error: String?
}

enum Health: String, Decodable, Sendable {
    case good, warn, bad, off
}

/// One parsed line of a card command; mirrors `collect::CardRow` in Rust.
struct CardRow: Decodable, Sendable {
    let health: Health?
    let name: String
    let detail: String?
    let section: Bool
    let percent: UInt8?

    private enum CodingKeys: String, CodingKey {
        case health, name, detail, section, percent
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        // An unrecognized health value degrades to no health rather than
        // failing the whole snapshot.
        health = try container.decodeIfPresent(String.self, forKey: .health).flatMap(Health.init(rawValue:))
        name = try container.decode(String.self, forKey: .name)
        detail = try container.decodeIfPresent(String.self, forKey: .detail)
        section = try container.decodeIfPresent(Bool.self, forKey: .section) ?? false
        percent = try container.decodeIfPresent(UInt8.self, forKey: .percent)
    }
}

enum AppError: LocalizedError {
    case message(String)

    var errorDescription: String? {
        switch self {
        case let .message(value): value
        }
    }
}

enum SnapshotRunner {
    static let timeout: TimeInterval = 20

    static func load() async throws -> Snapshot {
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

        let output = try await ProcessRun(process, timeout: timeout).output()
        guard output.status == 0 else {
            let message = String(decoding: output.stderr, as: UTF8.self)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw AppError.message(message.isEmpty ? "Usage refresh failed." : message)
        }
        return try JSONDecoder().decode(Snapshot.self, from: output.stdout)
    }
}

/// Runs a child process without parking a cooperative-pool thread: stdout and
/// stderr stream in through readability handlers, the exit status arrives via
/// the termination handler, and the caller resumes once all three are done.
/// If that takes longer than `timeout`, the process is terminated and the run
/// fails, even if a grandchild is still holding the pipes open.
fileprivate final class ProcessRun: @unchecked Sendable {
    struct Output: Sendable {
        let status: Int32
        let stdout: Data
        let stderr: Data
    }

    private let process: Process
    private let timeout: TimeInterval
    private let stdoutPipe = Pipe()
    private let stderrPipe = Pipe()

    // Everything below is guarded by `lock`.
    private let lock = NSLock()
    private var stdout = Data()
    private var stderr = Data()
    private var stdoutOpen = true
    private var stderrOpen = true
    private var status: Int32?
    private var continuation: CheckedContinuation<Output, any Error>?

    init(_ process: Process, timeout: TimeInterval) {
        self.process = process
        self.timeout = timeout
    }

    func output() async throws -> Output {
        try await withCheckedThrowingContinuation { continuation in
            locked { self.continuation = continuation }
            process.standardOutput = stdoutPipe
            process.standardError = stderrPipe
            stdoutPipe.fileHandleForReading.readabilityHandler = { [self] in receive($0, isError: false) }
            stderrPipe.fileHandleForReading.readabilityHandler = { [self] in receive($0, isError: true) }
            process.terminationHandler = { [self] in exited($0.terminationStatus) }
            do {
                try process.run()
            } catch {
                finish(.failure(error))
                return
            }
            DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout) { [self] in
                if process.isRunning { process.terminate() }
                finish(.failure(AppError.message("Usage refresh timed out.")))
            }
        }
    }

    private func receive(_ handle: FileHandle, isError: Bool) {
        let data = handle.availableData
        if data.isEmpty { handle.readabilityHandler = nil }
        let output = locked { () -> Output? in
            switch (isError, data.isEmpty) {
            case (false, true): stdoutOpen = false
            case (true, true): stderrOpen = false
            case (false, false): stdout.append(data)
            case (true, false): stderr.append(data)
            }
            return completed
        }
        if let output { finish(.success(output)) }
    }

    private func exited(_ code: Int32) {
        if let output = locked({ () -> Output? in status = code; return completed }) {
            finish(.success(output))
        }
    }

    /// The finished output, once the process exited and both pipes hit EOF.
    /// Call with `lock` held.
    private var completed: Output? {
        guard let status, !stdoutOpen, !stderrOpen else { return nil }
        return Output(status: status, stdout: stdout, stderr: stderr)
    }

    /// Resumes the caller exactly once; later calls are no-ops.
    private func finish(_ result: Result<Output, any Error>) {
        guard let continuation = locked({ () -> CheckedContinuation<Output, any Error>? in
            defer { self.continuation = nil }
            return self.continuation
        }) else { return }
        stdoutPipe.fileHandleForReading.readabilityHandler = nil
        stderrPipe.fileHandleForReading.readabilityHandler = nil
        continuation.resume(with: result)
    }

    private func locked<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }
}
