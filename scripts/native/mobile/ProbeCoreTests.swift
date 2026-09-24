import Foundation

protocol ProbeCoreHost {
    func start(configJson: String) throws -> String
    func stop(coreId: String) throws
}
enum ProbeCoreError: Error { case Failed(detail: String); case Unsupported }
enum VoyaContainer {
    static func dataDirectory() -> URL { URL(fileURLWithPath: CommandLine.arguments[1]) }
}

@main struct ProbeCoreTests {
    static func main() throws {
        let host = LibboxProbeCoreHost()
        let directory = VoyaContainer.dataDirectory().appendingPathComponent("PTest")
        let config = "{\"inbounds\":[{\"type\":\"socks\",\"listen\":\"127.0.0.1\",\"listen_port\":\(CommandLine.arguments[2])}],\"outbounds\":[{\"type\":\"direct\"}]}"
        // The container prefix itself is longer than sockaddr_un.sun_path.
        precondition(directory.path.utf8.count > 104)
        do {
            _ = try host.start(configJson: "not JSON")
            fatalError("invalid configuration was accepted")
        } catch {}
        let remaining = try FileManager.default.contentsOfDirectory(atPath: directory.path)
        precondition(remaining.isEmpty)
        for _ in 0..<3 {
            let id = try host.start(configJson: config)
            do {
                _ = try host.start(configJson: config)
                fatalError("overlapping start was accepted")
            } catch {}
            let runs = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            precondition(runs.count == 1)
            precondition(!FileManager.default.fileExists(atPath: runs[0].appendingPathComponent("command.sock").path))
            try host.stop(coreId: id)
            try host.stop(coreId: id)
            let remaining = try FileManager.default.contentsOfDirectory(atPath: directory.path)
            precondition(remaining.isEmpty)
        }
        print("PASS: long container path; failed start cleanup; overlap rejection; idempotent stop; port reuse; empty work directory")
    }
}
