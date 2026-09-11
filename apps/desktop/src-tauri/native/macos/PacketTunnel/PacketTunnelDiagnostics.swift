import Foundation
import os.log

private let providerStatusRelativePath = "Library/Application Support/VoyaVPN/packet-tunnel-status.json"
private let providerLogRelativePath = "Library/Application Support/VoyaVPN/provider.log"
private let providerLogMaxBytes = 512 * 1024
/// Bytes carried into the fresh generation when the log rotates. The host tails
/// `provider.log` for diagnostics, so a rotation must not blank the evidence.
private let providerLogCarryOverBytes = 64 * 1024

/// Owns status persistence and serialized, bounded provider logging.
final class PacketTunnelDiagnostics {
    static let shared = PacketTunnelDiagnostics()
    private let containerURL: () -> URL?

    init(containerURL: @escaping () -> URL? = PacketTunnelRuntime.containerURL) {
        self.containerURL = containerURL
    }

    deinit {
        try? providerLogHandle?.close()
    }

    private let logger = Logger(subsystem: "app.voyavpn.desktop.PacketTunnel", category: "PacketTunnelProvider")
    private var providerStatusURLOverride: URL?
    private var providerLogURLOverride: URL?
    /// Serializes provider-log appends. libbox forwards every sing-box log line
    /// here from arbitrary Go threads, and the handle, byte counter and
    /// formatter below are shared mutable state.
    private let providerLogQueue = DispatchQueue(label: "app.voyavpn.desktop.PacketTunnel.log")
    private let providerLogTimestampFormatter = ISO8601DateFormatter()
    private var providerLogHandle: FileHandle?
    private var providerLogBytes = 0

    func configure(_ runtimeConfig: PacketTunnelRuntimeConfig) {
        providerStatusURLOverride = containedDiagnosticsURL(runtimeConfig.statusPath, kind: "status")
        providerLogURLOverride = containedDiagnosticsURL(runtimeConfig.logPath, kind: "log")
        // The cached log handle belongs to the previous destination; drop it so
        // the next line opens the one this run was configured with.
        providerLogQueue.sync { closeProviderLog() }
    }

    /// Accept a host-supplied diagnostics path only when it resolves inside the
    /// App Group container.
    ///
    /// The runtime config comes from a file any process running as the user can
    /// write, while the provider itself can run as root in the system-extension
    /// packaging, so an unchecked absolute path would let the caller pick which
    /// file the provider creates and overwrites. Anything outside the container
    /// falls back to the container default.
    func containedDiagnosticsURL(_ rawPath: String?, kind: String) -> URL? {
        guard let path = rawPath?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty else {
            return nil
        }
        guard let containerURL = containerURL() else {
            return nil
        }

        let candidate = resolvedDestination(URL(fileURLWithPath: path))
        let container = containerURL.resolvingSymlinksInPath().standardizedFileURL
        let containerPrefix = container.path.hasSuffix("/") ? container.path : container.path + "/"
        guard candidate.path.hasPrefix(containerPrefix) else {
            logger.error(
                "ignoring PacketTunnel \(kind, privacy: .public) path outside the App Group container: \(path, privacy: .public)"
            )
            return nil
        }

        return candidate
    }

    /// Foundation may leave symlink ancestors unresolved when the destination
    /// file does not exist yet. Resolve the existing ancestor before appending
    /// the missing components so newly created logs obey the same boundary.
    private func resolvedDestination(_ url: URL) -> URL {
        var ancestor = url
        var missingComponents: [String] = []
        while !FileManager.default.fileExists(atPath: ancestor.path), ancestor.path != "/" {
            missingComponents.append(ancestor.lastPathComponent)
            ancestor.deleteLastPathComponent()
        }
        var resolved = ancestor.resolvingSymlinksInPath()
        for component in missingComponents.reversed() {
            resolved.appendPathComponent(component)
        }
        return resolved.standardizedFileURL
    }

    private func providerStatusURL() throws -> URL {
        if let providerStatusURLOverride {
            return providerStatusURLOverride
        }
        guard let containerURL = containerURL() else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        return containerURL.appendingPathComponent(providerStatusRelativePath)
    }

    private func providerLogURL() throws -> URL {
        if let providerLogURLOverride {
            return providerLogURLOverride
        }
        guard let containerURL = containerURL() else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        return containerURL.appendingPathComponent(providerLogRelativePath)
    }

    func writeStatus(
        state: String,
        lastError: String? = nil,
        breadcrumb: String
    ) {
        do {
            let url = try providerStatusURL()
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let breadcrumbs = providerBreadcrumbs(from: url, appending: breadcrumb)
            var object: [String: Any] = [
                "state": state,
                "providerBundlePath": Bundle.main.bundleURL.path,
                "breadcrumbs": breadcrumbs,
                "updatedAt": ISO8601DateFormatter().string(from: Date()),
            ]
            if let lastError, !lastError.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                object["lastError"] = lastError
            }
            let data = try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: url, options: .atomic)
            appendProviderLog("\(state): \(breadcrumb)")
        } catch {
            logger.error("failed to write PacketTunnel status: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func providerBreadcrumbs(from url: URL, appending breadcrumb: String) -> [String] {
        var breadcrumbs: [String] = []
        if let data = try? Data(contentsOf: url),
           let object = try? JSONSerialization.jsonObject(with: data),
           let dictionary = object as? [String: Any],
           let existing = dictionary["breadcrumbs"] as? [String]
        {
            breadcrumbs = existing
        }
        breadcrumbs.append(breadcrumb)
        return Array(breadcrumbs.suffix(20))
    }

    /// Appends one line to the provider log.
    ///
    /// This sits on the sing-box logging path inside the process that carries
    /// every packet, so it must not cost anything proportional to the file:
    /// the handle stays open and is appended to, and the file is rotated once
    /// it crosses the cap instead of being read back, trimmed and atomically
    /// rewritten per line. The write stays synchronous on a serial queue so a
    /// log burst applies backpressure instead of growing an unbounded backlog
    /// inside the tunnel process.
    func appendProviderLog(_ message: String) {
        let timestamp = Date()
        providerLogQueue.sync {
            writeProviderLogLine(timestamp: timestamp, message: message)
        }
    }

    private func writeProviderLogLine(timestamp: Date, message: String) {
        let line = "\(providerLogTimestampFormatter.string(from: timestamp)) \(message)\n"
        guard let data = line.data(using: .utf8) else {
            return
        }

        do {
            let url = try providerLogURL()
            try providerLogFileHandle(url).write(contentsOf: data)
            providerLogBytes += data.count
            if providerLogBytes > providerLogMaxBytes {
                try rotateProviderLog(url)
            }
        } catch {
            // Drop the handle so the next line re-opens instead of retrying
            // against a descriptor whose file may have been replaced.
            closeProviderLog()
            logger.error("failed to write PacketTunnel log: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func providerLogFileHandle(_ url: URL) throws -> FileHandle {
        if let providerLogHandle {
            return providerLogHandle
        }

        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
        let handle = try FileHandle(forWritingTo: url)
        providerLogBytes = Int(try handle.seekToEnd())
        providerLogHandle = handle
        return handle
    }

    /// Moves the current generation aside and starts a new one, keeping the most
    /// recent lines so the host's log tail survives the rotation.
    private func rotateProviderLog(_ url: URL) throws {
        closeProviderLog()
        let rotated = url.appendingPathExtension("1")
        try? FileManager.default.removeItem(at: rotated)
        try FileManager.default.moveItem(at: url, to: rotated)

        let carryOver = (try? Data(contentsOf: rotated))
            .map { Data($0.suffix(providerLogCarryOverBytes)) } ?? Data()
        FileManager.default.createFile(atPath: url.path, contents: carryOver)
    }

    /// Closes the cached handle; `providerLogBytes` is recomputed from the file
    /// the next time one is opened.
    private func closeProviderLog() {
        try? providerLogHandle?.close()
        providerLogHandle = nil
        providerLogBytes = 0
    }
}
