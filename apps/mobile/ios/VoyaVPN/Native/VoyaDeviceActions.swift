import AVFoundation
import Foundation
import PhotosUI
import React
import UIKit
import Vision

/// Device UX only. Business commands remain on VoyaNative's Rust envelope.
@objc(VoyaDeviceActions)
final class VoyaDeviceActions: NSObject, PHPickerViewControllerDelegate {
    private var resolvePending: RCTPromiseResolveBlock?
    private var rejectPending: RCTPromiseRejectBlock?
    @objc static func requiresMainQueueSetup() -> Bool { true }
    @objc var methodQueue: DispatchQueue { .main }

    private func claim(_ resolve: @escaping RCTPromiseResolveBlock, _ reject: @escaping RCTPromiseRejectBlock) -> Bool {
        guard resolvePending == nil else { reject("busy", "A device action is already open", nil); return false }
        resolvePending = resolve
        rejectPending = reject
        return true
    }

    private func finish(_ values: [String]?, code: String? = nil) {
        let resolve = resolvePending
        let reject = rejectPending
        resolvePending = nil
        rejectPending = nil
        if let code { reject?(code, code, nil) } else { resolve?(values) }
    }

    @objc func setConnectionShortcuts(_ connectLabel: String, disconnectLabel: String, resolve: @escaping RCTPromiseResolveBlock, reject: @escaping RCTPromiseRejectBlock) {
        UIApplication.shared.shortcutItems = [
            UIApplicationShortcutItem(type: "voyavpn.connect", localizedTitle: connectLabel, localizedSubtitle: nil, icon: UIApplicationShortcutIcon(systemImageName: "power"), userInfo: nil),
            UIApplicationShortcutItem(type: "voyavpn.disconnect", localizedTitle: disconnectLabel, localizedSubtitle: nil, icon: UIApplicationShortcutIcon(systemImageName: "stop.circle"), userInfo: nil)
        ]
        resolve(nil)
    }

    @objc func scanQr(_ cancelLabel: String, resolve: @escaping RCTPromiseResolveBlock, reject: @escaping RCTPromiseRejectBlock) {
        guard claim(resolve, reject) else { return }
        AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
            DispatchQueue.main.async {
                guard let self else { return }
                guard granted else { self.finish(nil, code: "cameraDenied"); return }
                // No camera — a simulator, or one switched off by a profile —
                // is known before anything is shown; a scanner presented only
                // to dismiss itself would flash a black screen at the user.
                guard AVCaptureDevice.default(for: .video) != nil,
                      let presenter = RCTPresentedViewController() else { self.finish(nil, code: "unavailable"); return }
                let scanner = VoyaQRScanner(cancelLabel: cancelLabel) { [weak self] value, code in
                    self?.finish(value.map { [$0] }, code: code)
                }
                scanner.modalPresentationStyle = .fullScreen
                presenter.present(scanner, animated: true)
            }
        }
    }

    @objc func pickQr(_ resolve: @escaping RCTPromiseResolveBlock, reject: @escaping RCTPromiseRejectBlock) {
        guard claim(resolve, reject) else { return }
        guard let presenter = RCTPresentedViewController() else { finish(nil, code: "unavailable"); return }
        var configuration = PHPickerConfiguration()
        configuration.filter = .images
        configuration.selectionLimit = 1
        let picker = PHPickerViewController(configuration: configuration)
        picker.delegate = self
        presenter.present(picker, animated: true)
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard let provider = results.first?.itemProvider else { finish(nil); return }
        provider.loadObject(ofClass: UIImage.self) { [weak self] object, _ in
            guard let image = object as? UIImage, let cgImage = image.cgImage else {
                DispatchQueue.main.async { self?.finish(nil, code: "noQr") }; return
            }
            let request = VNDetectBarcodesRequest()
#if targetEnvironment(simulator)
            // The simulator cannot create the inference context the later
            // revisions need. A device takes the newest its system has.
            request.revision = VNDetectBarcodesRequestRevision1
#endif
            request.symbologies = [.qr]
            do {
                let orientation: CGImagePropertyOrientation = switch image.imageOrientation {
                case .up: .up
                case .down: .down
                case .left: .left
                case .right: .right
                case .upMirrored: .upMirrored
                case .downMirrored: .downMirrored
                case .leftMirrored: .leftMirrored
                case .rightMirrored: .rightMirrored
                @unknown default: .up
                }
                try VNImageRequestHandler(cgImage: cgImage, orientation: orientation).perform([request])
                var seen = Set<String>()
                let values = (request.results ?? []).compactMap(\.payloadStringValue).filter { seen.insert($0).inserted }
                DispatchQueue.main.async { self?.finish(values, code: values.isEmpty ? "noQr" : nil) }
            } catch { DispatchQueue.main.async { self?.finish(nil, code: "noQr") } }
        }
    }

    @objc func shareDiagnostics(_ text: String, resolve: @escaping RCTPromiseResolveBlock, reject: @escaping RCTPromiseRejectBlock) {
        guard let presenter = RCTPresentedViewController() else { reject("unavailable", "No presentation context", nil); return }
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("VoyaDiagnostics", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            for old in try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.contentModificationDateKey]) {
                let date = try old.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate ?? .distantPast
                if date < Date().addingTimeInterval(-86400) { try? FileManager.default.removeItem(at: old) }
            }
            let file = directory.appendingPathComponent("diagnostics-\(UUID().uuidString).txt")
            try text.write(to: file, atomically: true, encoding: .utf8)
            let sheet = UIActivityViewController(activityItems: [file], applicationActivities: nil)
            sheet.popoverPresentationController?.sourceView = presenter.view
            sheet.popoverPresentationController?.sourceRect = CGRect(x: presenter.view.bounds.midX, y: presenter.view.bounds.midY, width: 1, height: 1)
            // UIKit calls this once per activity the user backs out of, with
            // the sheet still up — cancelling a mail composer returns to the
            // sheet. Only the call that ends the sheet settles the promise and
            // removes the file: settling twice is a React Native error, and an
            // activity chosen next would find its file gone. The sheet has
            // ended when an activity completed, when it was dismissed with no
            // activity chosen, or when it failed.
            var settled = false
            sheet.completionWithItemsHandler = { activityType, completed, _, error in
                guard !settled, completed || activityType == nil || error != nil else { return }
                settled = true
                try? FileManager.default.removeItem(at: file)
                if let error { reject("shareFailed", error.localizedDescription, error) } else { resolve(nil) }
            }
            presenter.present(sheet, animated: true)
        } catch { reject("shareFailed", error.localizedDescription, error) }
    }

    @objc func appVersion(_ resolve: RCTPromiseResolveBlock, reject _: RCTPromiseRejectBlock) {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? ""
        resolve("\(version) (\(build))")
    }
}

private final class VoyaQRScanner: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "app.voyavpn.qr-camera")
    private var preview: AVCaptureVideoPreviewLayer?
    private let completion: (String?, String?) -> Void
    private let cancelLabel: String
    private var finished = false
    /// Whether the presentation has finished; a dismissal asked for before
    /// then is ignored by UIKit.
    private var appeared = false
    /// What the scan came to while the scanner was still animating in.
    private var outcomeBeforeAppearing: (value: String?, code: String?)?

    init(cancelLabel: String, completion: @escaping (String?, String?) -> Void) {
        self.cancelLabel = cancelLabel
        self.completion = completion
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        view.layer.addSublayer(layer)
        preview = layer
        let cancel = UIButton(type: .system)
        cancel.setTitle(cancelLabel, for: .normal)
        cancel.titleLabel?.font = .preferredFont(forTextStyle: .headline)
        cancel.titleLabel?.adjustsFontForContentSizeCategory = true
        cancel.backgroundColor = .systemBackground
        cancel.layer.cornerRadius = 16
        cancel.addTarget(self, action: #selector(cancelScan), for: .touchUpInside)
        cancel.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(cancel)
        NSLayoutConstraint.activate([
            cancel.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 24),
            cancel.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -24),
            cancel.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -24),
            cancel.heightAnchor.constraint(greaterThanOrEqualToConstant: 52)
        ])
        queue.async { [weak self] in
            guard let self, let device = AVCaptureDevice.default(for: .video),
                  let input = try? AVCaptureDeviceInput(device: device) else {
                DispatchQueue.main.async { self?.complete(nil, code: "unavailable") }; return
            }
            let output = AVCaptureMetadataOutput()
            self.session.beginConfiguration()
            guard self.session.canAddInput(input), self.session.canAddOutput(output) else {
                self.session.commitConfiguration()
                DispatchQueue.main.async { self.complete(nil, code: "unavailable") }; return
            }
            self.session.addInput(input)
            self.session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main)
            output.metadataObjectTypes = [.qr]
            self.session.commitConfiguration()
            self.session.startRunning()
        }
    }
    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); preview?.frame = view.bounds }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        appeared = true
        if let outcome = outcomeBeforeAppearing {
            outcomeBeforeAppearing = nil
            report(outcome.value, code: outcome.code)
        }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        queue.async { [session] in session.stopRunning() }
    }
    @objc private func cancelScan() { complete(nil) }
    private func complete(_ value: String?, code: String? = nil) {
        guard !finished else { return }
        finished = true
        queue.async { [session] in session.stopRunning() }
        // The camera can fail within the presentation animation. Dismissing
        // then does nothing, the completion would never run, and the module
        // would answer "busy" to every scan until the app was relaunched.
        guard appeared else { outcomeBeforeAppearing = (value, code); return }
        report(value, code: code)
    }
    private func report(_ value: String?, code: String?) {
        dismiss(animated: true) { self.completion(value, code) }
    }
    func metadataOutput(_: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from _: AVCaptureConnection) {
        if let value = objects.compactMap({ ($0 as? AVMetadataMachineReadableCodeObject)?.stringValue }).first { complete(value) }
    }
}
