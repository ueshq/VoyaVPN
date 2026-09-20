import Foundation

/// Runs in a temporary directory, without loading or registering a NetworkExtension.
@main
private enum PacketTunnelTests {
    private struct Failure: Error { let message: String }

    private static func expect(_ condition: @autoclosure () throws -> Bool, _ message: String) throws {
        if try !condition() { throw Failure(message: message) }
    }

    private static func config(version: Int = 1, json: String = "{}", status: URL? = nil, log: URL? = nil) -> PacketTunnelRuntimeConfig {
        PacketTunnelRuntimeConfig(version: version, activeProfileId: "test-profile", mainConfigPath: "config.json",
                                  statusPath: status?.path, logPath: log?.path, singboxConfigJson: json)
    }

    static func main() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("voya-packet-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try validation()
        try optionsPrecedeFile(root)
        try workingPaths()
        try appGroupIsABundleFact()
        try diagnosticsContainment(root)
        try diagnosticsStatus(root)
        try diagnosticsRotation(root)
        try diagnosticsDestinationChange(root)
        try diagnosticsFallback(root)
        print("PacketTunnel runtime and diagnostics: 9 tests passed.")
    }

    private static func validation() throws {
        try PacketTunnelRuntime.validate(config())
        do {
            try PacketTunnelRuntime.validate(config(version: 2))
            throw Failure(message: "accepted unsupported version")
        } catch PacketTunnelProviderError.unsupportedRuntimeConfig(2) {}
        do {
            try PacketTunnelRuntime.validate(config(json: " \n\t"))
            throw Failure(message: "accepted empty config")
        } catch PacketTunnelProviderError.emptyRuntimeConfig {}
    }

    private static func optionsPrecedeFile(_ root: URL) throws {
        let file = root.appendingPathComponent("runtime.json")
        try JSONEncoder().encode(config(json: "file")).write(to: file)
        let data = try JSONEncoder().encode(config(json: "options"))
        let fromData = try PacketTunnelRuntime.loadRuntimeConfig(options: ["runtimeConfigJson": data as NSData], configURL: file)
        let fromString = try PacketTunnelRuntime.loadRuntimeConfig(options: ["runtimeConfigJson": String(decoding: data, as: UTF8.self) as NSString], configURL: file)
        let fromFile = try PacketTunnelRuntime.loadRuntimeConfig(configURL: file)
        try expect(fromData.singboxConfigJson == "options" && fromString.singboxConfigJson == "options", "options must win")
        try expect(fromFile.singboxConfigJson == "file", "missing options must load file")
        do {
            _ = try PacketTunnelRuntime.loadRuntimeConfig(options: ["runtimeConfigJson": "malformed" as NSString], configURL: file)
            throw Failure(message: "invalid options silently fell back to file")
        } catch is DecodingError {}
    }

    /// macOS and iOS ship these sources with different App Groups, so the
    /// identifier comes from the extension's own `Info.plist`. This test binary
    /// declares none, which is what an extension built without the key looks
    /// like: there is no container, and asking for paths fails closed rather
    /// than reaching into some other app's group.
    private static func appGroupIsABundleFact() throws {
        try expect(PacketTunnelRuntime.appGroupIdentifier() == nil, "an undeclared App Group must not resolve")
        try expect(PacketTunnelRuntime.containerURL() == nil, "no App Group means no container")
        do {
            _ = try PacketTunnelRuntime.runtimePaths(containerURL: nil)
            throw Failure(message: "paths resolved without an App Group container")
        } catch PacketTunnelProviderError.missingAppGroupContainer {}
    }

    private static func workingPaths() throws {
        let paths = try PacketTunnelRuntime.runtimePaths(containerURL: URL(fileURLWithPath: "/tmp/group"))
        try expect(paths.baseURL.path == "/tmp/group/PT", "short base path changed")
        try expect(paths.workingURL.lastPathComponent == "Working" && paths.tempURL.lastPathComponent == "Temp", "working paths changed")
        do {
            _ = try PacketTunnelRuntime.runtimePaths(containerURL: nil)
            throw Failure(message: "accepted missing container")
        } catch PacketTunnelProviderError.missingAppGroupContainer {}
        let suffixBytes = "/PT/command.sock".utf8.count
        _ = try PacketTunnelRuntime.runtimePaths(containerURL: URL(fileURLWithPath: "/" + String(repeating: "a", count: 102 - suffixBytes)))
        do {
            _ = try PacketTunnelRuntime.runtimePaths(containerURL: URL(fileURLWithPath: "/" + String(repeating: "a", count: 103 - suffixBytes)))
            throw Failure(message: "accepted 104-byte socket path")
        } catch PacketTunnelProviderError.libboxBasePathTooLong {}
        do {
            _ = try PacketTunnelRuntime.runtimePaths(containerURL: URL(fileURLWithPath: "/" + String(repeating: "界", count: 32)))
            throw Failure(message: "socket limit must count UTF-8 bytes")
        } catch PacketTunnelProviderError.libboxBasePathTooLong {}
    }

    private static func diagnosticsContainment(_ root: URL) throws {
        let diagnostics = PacketTunnelDiagnostics(containerURL: { root })
        let child = root.appendingPathComponent("inside/status.json")
        try expect(diagnostics.containedDiagnosticsURL(child.path, kind: "test") == child.resolvingSymlinksInPath(), "rejected child path")
        for path in [root.path, root.path + "-sibling/status.json", root.path + "/../escape.json", " "] {
            try expect(diagnostics.containedDiagnosticsURL(path, kind: "test") == nil, "accepted escaping or empty path")
        }
        let link = root.appendingPathComponent("outside-link")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: root.deletingLastPathComponent())
        try expect(diagnostics.containedDiagnosticsURL(link.appendingPathComponent("escape.json").path, kind: "test") == nil, "accepted escaping symlink")
        let absent = PacketTunnelDiagnostics(containerURL: { nil })
        try expect(absent.containedDiagnosticsURL(child.path, kind: "test") == nil, "accepted path without container")
    }

    private static func diagnosticsStatus(_ root: URL) throws {
        let diagnostics = PacketTunnelDiagnostics(containerURL: { root })
        let status = root.appendingPathComponent("status.json")
        diagnostics.configure(config(status: status, log: root.appendingPathComponent("status.log")))
        for index in 0..<25 {
            diagnostics.writeStatus(state: "starting", breadcrumb: "step-\(index)")
        }
        diagnostics.writeStatus(state: "failed", lastError: "test failure", breadcrumb: "failure")
        let object = try JSONSerialization.jsonObject(with: Data(contentsOf: status)) as? [String: Any]
        let breadcrumbs = object?["breadcrumbs"] as? [String]
        try expect(breadcrumbs?.count == 20 && breadcrumbs?.first == "step-6" && breadcrumbs?.last == "failure", "breadcrumb history must retain last 20 entries")
        try expect(object?["state"] as? String == "failed" && object?["lastError"] as? String == "test failure", "status lost failure")
        diagnostics.writeStatus(state: "running", breadcrumb: "recovered")
        let recovered = try JSONSerialization.jsonObject(with: Data(contentsOf: status)) as? [String: Any]
        try expect(recovered?["lastError"] == nil, "recovered status retained stale error")
    }

    private static func diagnosticsRotation(_ root: URL) throws {
        let diagnostics = PacketTunnelDiagnostics(containerURL: { root })
        let log = root.appendingPathComponent("rotate.log")
        diagnostics.configure(config(log: log))
        diagnostics.appendProviderLog(String(repeating: "x", count: 512 * 1024) + "carry-over-marker")
        let rotated = log.appendingPathExtension("1")
        try expect(FileManager.default.fileExists(atPath: rotated.path), "log did not rotate")
        try expect(Data(contentsOf: log).count == 64 * 1024, "rotation did not preserve bounded tail")
        try expect(String(contentsOf: log, encoding: .utf8).contains("carry-over-marker"), "rotation lost newest evidence")
        diagnostics.appendProviderLog("after-rotation")
        try expect(String(contentsOf: log, encoding: .utf8).contains("after-rotation"), "did not reopen rotated log")
    }

    private static func diagnosticsDestinationChange(_ root: URL) throws {
        let diagnostics = PacketTunnelDiagnostics(containerURL: { root })
        let first = root.appendingPathComponent("first.log")
        let second = root.appendingPathComponent("second.log")
        diagnostics.configure(config(log: first))
        diagnostics.appendProviderLog("first-destination")
        diagnostics.configure(config(log: second))
        diagnostics.appendProviderLog("second-destination")
        try expect(!String(contentsOf: first, encoding: .utf8).contains("second-destination"), "reconfiguration reused old handle")
        try expect(String(contentsOf: second, encoding: .utf8).contains("second-destination"), "new destination received no logs")
    }

    private static func diagnosticsFallback(_ root: URL) throws {
        let diagnostics = PacketTunnelDiagnostics(containerURL: { root })
        let outside = root.deletingLastPathComponent().appendingPathComponent("forbidden-\(UUID().uuidString)")
        diagnostics.configure(config(status: outside, log: outside))
        diagnostics.writeStatus(state: "running", breadcrumb: "fallback")
        try expect(!FileManager.default.fileExists(atPath: outside.path), "wrote outside container")
        let defaults = root.appendingPathComponent("Library/Application Support/VoyaVPN")
        try expect(FileManager.default.fileExists(atPath: defaults.appendingPathComponent("packet-tunnel-status.json").path), "missing default status")
        try expect(String(contentsOf: defaults.appendingPathComponent("provider.log"), encoding: .utf8).contains("fallback"), "missing default log")
    }
}
