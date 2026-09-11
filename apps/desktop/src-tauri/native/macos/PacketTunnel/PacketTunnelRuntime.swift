import Foundation

private let appGroupIdentifier = "group.app.voyavpn.desktop"
private let runtimeConfigRelativePath = "Library/Application Support/VoyaVPN/packet-tunnel-runtime.json"

/// Host payload decoding, validation and short libbox working paths.
enum PacketTunnelRuntime {
    static func containerURL() -> URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier)
    }

    static func validate(_ runtimeConfig: PacketTunnelRuntimeConfig) throws {
        guard runtimeConfig.version == 1 else {
            throw PacketTunnelProviderError.unsupportedRuntimeConfig(runtimeConfig.version)
        }
        guard !runtimeConfig.singboxConfigJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw PacketTunnelProviderError.emptyRuntimeConfig
        }
    }

    static func loadRuntimeConfig(options: [String: NSObject]? = nil, configURL: URL? = nil) throws -> PacketTunnelRuntimeConfig {
        if let data = options?["runtimeConfigJson"] as? Data {
            return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
        }
        if let text = options?["runtimeConfigJson"] as? String,
           let data = text.data(using: .utf8)
        {
            return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
        }
        let data = try Data(contentsOf: configURL ?? runtimeConfigURL())
        return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
    }

    static func runtimePaths(containerURL: URL? = PacketTunnelRuntime.containerURL()) throws -> PacketTunnelRuntimePaths {
        guard let containerURL else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        // libbox binds a unix socket at "<basePath>/command.sock"; macOS caps
        // sockaddr_un.sun_path at 104 bytes (incl. NUL), so the base directory
        // must stay short. The container root plus "PT" keeps it well below
        // the limit, unlike Library/Application Support/... which exceeds it.
        let baseURL = containerURL.appendingPathComponent("PT", isDirectory: true)
        let commandSocketPath = baseURL.path + "/command.sock"
        guard commandSocketPath.utf8.count <= 103 else {
            throw PacketTunnelProviderError.libboxBasePathTooLong(commandSocketPath)
        }

        return PacketTunnelRuntimePaths(
            baseURL: baseURL,
            workingURL: baseURL.appendingPathComponent("Working", isDirectory: true),
            tempURL: baseURL.appendingPathComponent("Temp", isDirectory: true)
        )
    }

    static func runtimeConfigURL() throws -> URL {
        guard let containerURL = containerURL() else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        return containerURL.appendingPathComponent(runtimeConfigRelativePath)
    }
}

struct PacketTunnelRuntimePaths {
    let baseURL: URL
    let workingURL: URL
    let tempURL: URL
}

struct PacketTunnelRuntimeConfig: Codable {
    let version: Int
    let activeProfileId: String?
    let mainConfigPath: String
    let statusPath: String?
    let logPath: String?
    let singboxConfigJson: String
}

enum PacketTunnelProviderError: LocalizedError {
    case missingAppGroupContainer
    case emptyRuntimeConfig
    case unsupportedRuntimeConfig(Int)
    case singBoxRuntimeUnavailable
    case libboxBasePathTooLong(String)
    case libboxSetupFailed(String)
    case libboxCommandServerFailed(String)
    case libboxServiceFailed(String)

    var errorDescription: String? {
        switch self {
        case .missingAppGroupContainer:
            return "VoyaVPN App Group container is unavailable."
        case .emptyRuntimeConfig:
            return "VoyaVPN PacketTunnel runtime config is empty."
        case .unsupportedRuntimeConfig(let version):
            return "VoyaVPN PacketTunnel runtime config version \(version) is not supported."
        case .singBoxRuntimeUnavailable:
            return "VoyaVPN PacketTunnel requires the sing-box Apple/libbox runtime. Build it with `pnpm native:macos:libbox` or set VOYAVPN_LIBBOX_FRAMEWORK."
        case .libboxBasePathTooLong(let path):
            return "VoyaVPN PacketTunnel libbox command socket path exceeds the macOS 104-byte sun_path limit: \(path)"
        case .libboxSetupFailed(let message):
            return "VoyaVPN PacketTunnel failed to set up libbox: \(message)"
        case .libboxCommandServerFailed(let message):
            return "VoyaVPN PacketTunnel failed to create libbox command server: \(message)"
        case .libboxServiceFailed(let message):
            return "VoyaVPN PacketTunnel failed to start sing-box service: \(message)"
        }
    }
}
