import Foundation

/// The `Info.plist` key each platform's extension declares its App Group in.
///
/// macOS and iOS are separate apps with separate identifiers
/// (`group.app.voyavpn.desktop` and `group.app.voyavpn.mobile`), and macOS
/// elects providers globally by bundle id, so the two must never share one.
/// These sources are shared, so the identifier is read from the bundle the
/// provider was loaded from rather than compiled in.
private let appGroupInfoKey = "VoyaAppGroupIdentifier"

/// What this provider calls itself in the unified log and in queue labels.
enum PacketTunnelIdentity {
    /// The extension's own bundle id, which differs per platform
    /// (`app.voyavpn.desktop.PacketTunnel`, `app.voyavpn.mobile.PacketTunnel`),
    /// so a `log stream --predicate 'subsystem == …'` names the right app. The
    /// fallback is for the command-line test binary, which has no bundle.
    static let subsystem = Bundle.main.bundleIdentifier ?? "app.voyavpn.PacketTunnel"
}

/// Host payload decoding, validation and short libbox working paths.
enum PacketTunnelRuntime {
    /// The App Group this extension declares, or `nil` if it declares none.
    static func appGroupIdentifier(bundle: Bundle = .main) -> String? {
        guard let identifier = bundle.object(forInfoDictionaryKey: appGroupInfoKey) as? String,
              !identifier.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            return nil
        }

        return identifier
    }

    static func containerURL(bundle: Bundle = .main) -> URL? {
        guard let identifier = appGroupIdentifier(bundle: bundle) else { return nil }

        return FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier)
    }

    static func validate(_ runtimeConfig: PacketTunnelRuntimeConfig) throws {
        guard runtimeConfig.version == 1 else {
            throw PacketTunnelProviderError.unsupportedRuntimeConfig(runtimeConfig.version)
        }
        guard !runtimeConfig.singboxConfigJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw PacketTunnelProviderError.emptyRuntimeConfig
        }
    }

    /// The configuration the app handed over with the start request.
    ///
    /// It only ever arrives inline: neither app writes it to disk, because it
    /// holds the node's credentials. A start without it came from the system —
    /// the VPN switch in Settings, Control Center — which has no configuration
    /// to give, and says so.
    static func loadRuntimeConfig(options: [String: NSObject]?) throws -> PacketTunnelRuntimeConfig {
        if let data = options?["runtimeConfigJson"] as? Data {
            return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
        }
        if let text = options?["runtimeConfigJson"] as? String,
           let data = text.data(using: .utf8)
        {
            return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
        }
        throw PacketTunnelProviderError.startedOutsideApp
    }

    static func runtimePaths(containerURL: URL? = PacketTunnelRuntime.containerURL()) throws -> PacketTunnelRuntimePaths {
        guard let containerURL else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        // "PT" at the container root dates from when libbox bound its command
        // socket under the base path and the name had to fit `sun_path`. The
        // socket is no longer opened, but installed tunnels keep their cache
        // and working files here, so the directory stays where it is.
        let baseURL = containerURL.appendingPathComponent("PT", isDirectory: true)

        return PacketTunnelRuntimePaths(
            baseURL: baseURL,
            workingURL: baseURL.appendingPathComponent("Working", isDirectory: true),
            tempURL: baseURL.appendingPathComponent("Temp", isDirectory: true),
            stderrURL: baseURL.appendingPathComponent("stderr.log")
        )
    }

}

struct PacketTunnelRuntimePaths {
    let baseURL: URL
    let workingURL: URL
    let tempURL: URL
    /// Where libbox redirects the provider's stderr, so a Go panic that kills
    /// the process leaves its trace behind.
    let stderrURL: URL
}

/// What the provider reads of the handshake. The hosts send more — the node's
/// id, the config's path on their side — and decoding ignores what is not
/// declared here, so a field nothing reads cannot fail a start by being absent.
struct PacketTunnelRuntimeConfig: Codable {
    let version: Int
    let statusPath: String?
    let logPath: String?
    let singboxConfigJson: String
}

enum PacketTunnelProviderError: LocalizedError {
    case missingAppGroupContainer
    case emptyRuntimeConfig
    case unsupportedRuntimeConfig(Int)
    case singBoxRuntimeUnavailable
    case libboxSetupFailed(String)
    case libboxCommandServerFailed(String)
    case libboxServiceFailed(String)
    case startedOutsideApp

    var errorDescription: String? {
        switch self {
        case .startedOutsideApp:
            return "Open VoyaVPN and connect from the app. The system VPN switch cannot start this connection on its own."
        case .missingAppGroupContainer:
            return "VoyaVPN App Group container is unavailable."
        case .emptyRuntimeConfig:
            return "VoyaVPN PacketTunnel runtime config is empty."
        case .unsupportedRuntimeConfig(let version):
            return "VoyaVPN PacketTunnel runtime config version \(version) is not supported."
        case .singBoxRuntimeUnavailable:
            return "VoyaVPN PacketTunnel requires the sing-box Apple/libbox runtime. Build it with `vp run native:macos:libbox` or set VOYAVPN_LIBBOX_FRAMEWORK."
        case .libboxSetupFailed(let message):
            return "VoyaVPN PacketTunnel failed to set up libbox: \(message)"
        case .libboxCommandServerFailed(let message):
            return "VoyaVPN PacketTunnel failed to create libbox command server: \(message)"
        case .libboxServiceFailed(let message):
            return "VoyaVPN PacketTunnel failed to start sing-box service: \(message)"
        }
    }
}
