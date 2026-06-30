import Foundation
import NetworkExtension

private let appGroupIdentifier = "group.app.voyavpn.desktop"
private let providerBundleIdentifier = "app.voyavpn.desktop.PacketTunnel"
private let runtimeConfigRelativePath = "Library/Application Support/VoyaVPN/packet-tunnel-runtime.json"
private let localizedDescription = "VoyaVPN"

@main
struct VoyaPacketTunnelManager {
    static func main() async {
        do {
            try await run(arguments: Array(CommandLine.arguments.dropFirst()))
        } catch {
            fputs("\(error.localizedDescription)\n", stderr)
            exit(1)
        }
    }

    private static func run(arguments: [String]) async throws {
        guard let command = arguments.first else {
            throw TunnelCtlError.invalidArguments("missing command")
        }

        switch command {
        case "status":
            print(try await status())
        case "start":
            let request = try StartRequest(arguments: Array(arguments.dropFirst()))
            try writeRuntimeConfig(request)
            try await startTunnel()
            print("starting")
        case "stop":
            try await stopTunnel()
            print("stopped")
        case "--help", "-h":
            printHelp()
        default:
            throw TunnelCtlError.invalidArguments("unknown command: \(command)")
        }
    }

    private static func printHelp() {
        print("usage:")
        print("  voyavpn-macos-tunnelctl status")
        print("  voyavpn-macos-tunnelctl start --config <config.json> [--profile <id>]")
        print("  voyavpn-macos-tunnelctl stop")
    }

    private static func status() async throws -> String {
        guard let manager = try await loadManager(createIfMissing: false) else {
            return "permissionRequired"
        }

        switch manager.connection.status {
        case .connected:
            return "running"
        case .connecting, .reasserting, .disconnecting:
            return "starting"
        case .disconnected, .invalid:
            return "stopped"
        @unknown default:
            return "error"
        }
    }

    private static func startTunnel() async throws {
        guard let manager = try await loadManager(createIfMissing: true) else {
            throw TunnelCtlError.managerUnavailable
        }

        manager.isEnabled = true
        try await save(manager)
        try await load(manager)
        try manager.connection.startVPNTunnel()
    }

    private static func stopTunnel() async throws {
        guard let manager = try await loadManager(createIfMissing: false) else {
            return
        }

        manager.connection.stopVPNTunnel()
    }

    private static func loadManager(createIfMissing: Bool) async throws -> NETunnelProviderManager? {
        let managers = try await loadAllManagers()
        if let existing = managers.first(where: {
            guard let proto = $0.protocolConfiguration as? NETunnelProviderProtocol else {
                return false
            }
            return proto.providerBundleIdentifier == providerBundleIdentifier
        }) {
            configure(existing)
            return existing
        }
        if !createIfMissing {
            return nil
        }

        let manager = NETunnelProviderManager()
        configure(manager)
        return manager
    }

    private static func configure(_ manager: NETunnelProviderManager) {
        let proto = (manager.protocolConfiguration as? NETunnelProviderProtocol) ?? NETunnelProviderProtocol()
        proto.providerBundleIdentifier = providerBundleIdentifier
        proto.serverAddress = localizedDescription
        proto.providerConfiguration = [
            "runtimeConfigRelativePath": runtimeConfigRelativePath,
            "appGroupIdentifier": appGroupIdentifier,
        ]

        manager.localizedDescription = localizedDescription
        manager.protocolConfiguration = proto
    }

    private static func loadAllManagers() async throws -> [NETunnelProviderManager] {
        try await withCheckedThrowingContinuation { continuation in
            NETunnelProviderManager.loadAllFromPreferences { managers, error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: managers ?? [])
                }
            }
        }
    }

    private static func save(_ manager: NETunnelProviderManager) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            manager.saveToPreferences { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume()
                }
            }
        }
    }

    private static func load(_ manager: NETunnelProviderManager) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            manager.loadFromPreferences { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume()
                }
            }
        }
    }

    private static func writeRuntimeConfig(_ request: StartRequest) throws {
        guard request.configPath.path.hasPrefix("/") else {
            throw TunnelCtlError.invalidArguments("config path must be absolute")
        }

        let singboxConfigJson = try String(contentsOf: request.configPath, encoding: .utf8)
        let runtime = PacketTunnelRuntimeConfig(
            version: 1,
            activeProfileId: request.profileId,
            mainConfigPath: request.configPath.path,
            singboxConfigJson: singboxConfigJson
        )
        let data = try JSONEncoder().encode(runtime)
        let destination = try runtimeConfigURL()
        try FileManager.default.createDirectory(
            at: destination.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try data.write(to: destination, options: [.atomic])
    }

    private static func runtimeConfigURL() throws -> URL {
        guard let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            throw TunnelCtlError.missingAppGroupContainer
        }

        return container.appendingPathComponent(runtimeConfigRelativePath)
    }
}

private struct StartRequest {
    let configPath: URL
    let profileId: String?

    init(arguments: [String]) throws {
        var configPath: URL?
        var profileId: String?
        var index = 0
        while index < arguments.count {
            switch arguments[index] {
            case "--config":
                guard index + 1 < arguments.count else {
                    throw TunnelCtlError.invalidArguments("--config requires a path")
                }
                configPath = URL(fileURLWithPath: arguments[index + 1])
                index += 2
            case "--profile":
                guard index + 1 < arguments.count else {
                    throw TunnelCtlError.invalidArguments("--profile requires a value")
                }
                profileId = arguments[index + 1]
                index += 2
            default:
                throw TunnelCtlError.invalidArguments("unknown argument: \(arguments[index])")
            }
        }

        guard let configPath else {
            throw TunnelCtlError.invalidArguments("missing --config <path>")
        }

        self.configPath = configPath
        self.profileId = profileId
    }
}

private struct PacketTunnelRuntimeConfig: Codable {
    let version: Int
    let activeProfileId: String?
    let mainConfigPath: String
    let singboxConfigJson: String
}

private enum TunnelCtlError: LocalizedError {
    case invalidArguments(String)
    case managerUnavailable
    case missingAppGroupContainer

    var errorDescription: String? {
        switch self {
        case .invalidArguments(let message):
            return message
        case .managerUnavailable:
            return "VoyaVPN PacketTunnel manager is unavailable."
        case .missingAppGroupContainer:
            return "VoyaVPN App Group container is unavailable."
        }
    }
}
