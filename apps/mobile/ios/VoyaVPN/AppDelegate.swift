import ObjectiveC
import UIKit
import React
import React_RCTAppDelegate
import ReactAppDependencyProvider

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?

  var reactNativeDelegate: ReactNativeDelegate?
  var reactNativeFactory: RCTReactNativeFactory?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
#if DEBUG
    // Must run before any URLSession is created: RN's packager probe uses
    // sharedSession and the bundle download builds from defaultSessionConfiguration.
    DevURLSession.bypassSystemProxy()
#endif

    let delegate = ReactNativeDelegate()
    let factory = RCTReactNativeFactory(delegate: delegate)
    delegate.dependencyProvider = RCTAppDependencyProvider()

    reactNativeDelegate = delegate
    reactNativeFactory = factory

    window = UIWindow(frame: UIScreen.main.bounds)

    factory.startReactNative(
      withModuleName: "VoyaVPN",
      in: window,
      launchOptions: launchOptions
    )

    return true
  }
}

class ReactNativeDelegate: RCTDefaultReactNativeFactoryDelegate {
  override func sourceURL(for bridge: RCTBridge) -> URL? {
    self.bundleURL()
  }

  override func bundleURL() -> URL? {
#if DEBUG
    // Mint the Metro URL directly. The shared-settings helper probes /status
    // and returns nil when that probe fails — including when a system HTTP
    // proxy RSTs localhost — which surfaces as "No script URL provided".
    RCTBundleURLProvider.jsBundleURL(
      forBundleRoot: "index",
      packagerHost: "localhost:8081",
      enableDev: true,
      enableMinification: false,
      inlineSourceMap: false
    )
#else
    Bundle.main.url(forResource: "main", withExtension: "jsbundle")
#endif
  }
}

#if DEBUG
/// CFNetwork honours the Mac system HTTP proxy even for 127.0.0.1, so a local
/// Clash/V2Ray proxy turns Metro's /status into 502/RST and
/// `RCTBundleURLProvider` collapses the script URL to nil. Debug builds skip
/// proxy lookup entirely so the simulator can reach Metro with the proxy on.
enum DevURLSession {
  private static var didBypass = false

  static func bypassSystemProxy() {
    guard !didBypass else { return }
    didBypass = true
    swizzleConfigurationFactory(
      original: Selector(("defaultSessionConfiguration")),
      replacement: #selector(URLSessionConfiguration.voya_defaultSessionConfiguration)
    )
    swizzleConfigurationFactory(
      original: Selector(("ephemeralSessionConfiguration")),
      replacement: #selector(URLSessionConfiguration.voya_ephemeralSessionConfiguration)
    )
  }

  private static func swizzleConfigurationFactory(original: Selector, replacement: Selector) {
    let cls: AnyClass = URLSessionConfiguration.self
    guard
      let originalMethod = class_getClassMethod(cls, original),
      let replacementMethod = class_getClassMethod(cls, replacement)
    else {
      return
    }
    method_exchangeImplementations(originalMethod, replacementMethod)
  }
}

extension URLSessionConfiguration {
  @objc class func voya_defaultSessionConfiguration() -> URLSessionConfiguration {
    let config = voya_defaultSessionConfiguration()
    // Empty dictionary disables proxy lookups (CFNetwork documented behaviour).
    config.connectionProxyDictionary = [:]
    return config
  }

  @objc class func voya_ephemeralSessionConfiguration() -> URLSessionConfiguration {
    let config = voya_ephemeralSessionConfiguration()
    config.connectionProxyDictionary = [:]
    return config
  }
}
#endif
