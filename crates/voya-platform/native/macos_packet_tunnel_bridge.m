#import <Foundation/Foundation.h>
#import <NetworkExtension/NetworkExtension.h>
#import <SystemExtensions/SystemExtensions.h>
#import <dispatch/dispatch.h>
#import <stdatomic.h>
#import <stdint.h>
#import <stdlib.h>
#import <string.h>
#import "macos_tunnel_wait.h"

static NSString *const VoyaAppGroupIdentifier = @"group.app.voyavpn.desktop";
static NSString *const VoyaProviderBundleIdentifier = @"app.voyavpn.desktop.PacketTunnel";
static NSString *const VoyaLocalizedDescription = @"VoyaVPN";

static char *VoyaCopyCString(NSString *string) {
    const char *utf8 = [string UTF8String];
    if (utf8 == NULL) {
        utf8 = "";
    }
    return strdup(utf8);
}

static char *VoyaCopyError(NSError *error) {
    NSString *message = error.localizedDescription ?: @"unknown macOS PacketTunnel error";
    return VoyaCopyCString([@"error:" stringByAppendingString:message]);
}

static NSError *VoyaMakeError(NSString *message) {
    return [NSError errorWithDomain:@"VoyaVPNPacketTunnelBridge"
                               code:1
                           userInfo:@{NSLocalizedDescriptionKey: message}];
}

/// Upper bound for every NetworkExtension preference round trip.
///
/// `loadAllFromPreferences`/`saveToPreferences`/`loadFromPreferences` normally
/// answer in well under a second, but `nesessionmanager` can wedge (a stuck
/// approval dialog, a half-installed system extension). The Rust side calls
/// these synchronously from a supervisor task, so an unbounded wait would pin
/// that thread forever; a bounded one surfaces as a normal bridge error.
static const NSTimeInterval VoyaPreferencesTimeoutSeconds = 15.0;
static const NSTimeInterval VoyaLastDisconnectErrorTimeoutSeconds = 5.0;

static BOOL VoyaWaitWithTimeout(dispatch_semaphore_t semaphore, NSTimeInterval timeoutSeconds) {
    // Off the main thread nothing needs servicing while waiting, so the wait
    // ends the moment the reply arrives instead of at the next poll.
    if (![NSThread isMainThread]) {
        dispatch_time_t limit = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(timeoutSeconds * NSEC_PER_SEC));
        return dispatch_semaphore_wait(semaphore, limit) == 0;
    }
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:timeoutSeconds];
    while ([[NSDate date] compare:deadline] == NSOrderedAscending) {
        if (dispatch_semaphore_wait(semaphore, DISPATCH_TIME_NOW) == 0) {
            return YES;
        }
        NSDate *limit = [NSDate dateWithTimeIntervalSinceNow:0.05];
        [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:limit];
    }
    return NO;
}

@interface VoyaSystemExtensionActivationDelegate : NSObject <OSSystemExtensionRequestDelegate>
@property(nonatomic) dispatch_semaphore_t semaphore;
@property(nonatomic, copy) NSString *result;
@end

@implementation VoyaSystemExtensionActivationDelegate

- (instancetype)init {
    self = [super init];
    if (self != nil) {
        _semaphore = dispatch_semaphore_create(0);
    }
    return self;
}

- (void)completeWithResult:(NSString *)result {
    if (self.result.length == 0) {
        self.result = result;
        dispatch_semaphore_signal(self.semaphore);
    }
}

- (OSSystemExtensionReplacementAction)request:(OSSystemExtensionRequest *)request
               actionForReplacingExtension:(OSSystemExtensionProperties *)existing
                              withExtension:(OSSystemExtensionProperties *)extension {
    (void)request;
    (void)existing;
    (void)extension;
    return OSSystemExtensionReplacementActionReplace;
}

- (void)requestNeedsUserApproval:(OSSystemExtensionRequest *)request {
    (void)request;
    [self completeWithResult:@"permissionRequired:Approve the VoyaVPN PacketTunnel system extension in System Settings, then enable TUN again."];
}

- (void)request:(OSSystemExtensionRequest *)request didFailWithError:(NSError *)error {
    (void)request;
    NSString *message = error.localizedDescription ?: @"unknown System Extension activation error";
    [self completeWithResult:[@"error:" stringByAppendingString:message]];
}

- (void)request:(OSSystemExtensionRequest *)request didFinishWithResult:(OSSystemExtensionRequestResult)result {
    (void)request;
    switch (result) {
        case OSSystemExtensionRequestCompleted:
            [self completeWithResult:@"ok"];
            break;
        case OSSystemExtensionRequestWillCompleteAfterReboot:
            [self completeWithResult:@"error:VoyaVPN PacketTunnel system extension update will complete after reboot."];
            break;
    }
}

@end

static NSURL *VoyaSystemExtensionBundleURL(void) {
    return [[[NSBundle mainBundle] bundleURL]
        URLByAppendingPathComponent:@"Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension"
                        isDirectory:YES];
}

static BOOL VoyaSystemExtensionIsBundled(void) {
    NSURL *url = VoyaSystemExtensionBundleURL();
    return url != nil && [[NSFileManager defaultManager] fileExistsAtPath:url.path];
}

static NSString *VoyaEnsureSystemExtensionActivated(void) {
    if (!VoyaSystemExtensionIsBundled()) {
        return nil;
    }

    VoyaSystemExtensionActivationDelegate *delegate = [[VoyaSystemExtensionActivationDelegate alloc] init];
    OSSystemExtensionRequest *request =
        [OSSystemExtensionRequest activationRequestForExtension:VoyaProviderBundleIdentifier
                                                          queue:dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0)];
    request.delegate = delegate;
    [[OSSystemExtensionManager sharedManager] submitRequest:request];

    if (!VoyaWaitWithTimeout(delegate.semaphore, 20.0)) {
        return @"error:Timed out waiting for VoyaVPN PacketTunnel system extension activation.";
    }
    if ([delegate.result isEqualToString:@"ok"]) {
        return nil;
    }
    return delegate.result ?: @"error:unknown System Extension activation result";
}

static NSArray<NETunnelProviderManager *> *VoyaLoadAllManagers(NSError **outError) {
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block NSArray<NETunnelProviderManager *> *loadedManagers = nil;
    __block NSError *loadedError = nil;

    [NETunnelProviderManager loadAllFromPreferencesWithCompletionHandler:
        ^(NSArray<NETunnelProviderManager *> *managers, NSError *error) {
            loadedManagers = managers ?: @[];
            loadedError = error;
            dispatch_semaphore_signal(semaphore);
        }];
    // The completion block writes only `__block` storage, which outlives this
    // frame, so a late reply after the timeout stays safe; the timed-out values
    // are simply never read.
    if (!VoyaWaitWithTimeout(semaphore, VoyaPreferencesTimeoutSeconds)) {
        if (outError != NULL) {
            *outError = VoyaMakeError(@"Timed out loading the VoyaVPN VPN configuration from system preferences.");
        }
        return nil;
    }

    if (loadedError != nil && outError != NULL) {
        *outError = loadedError;
    }
    return loadedError == nil ? loadedManagers : nil;
}

/// Returns whether anything had to change, which is whether the configuration
/// has to be saved before it can be used.
static BOOL VoyaConfigureManager(NETunnelProviderManager *manager) {
    NETunnelProviderProtocol *proto = nil;
    BOOL changed = NO;
    if ([manager.protocolConfiguration isKindOfClass:[NETunnelProviderProtocol class]]) {
        proto = (NETunnelProviderProtocol *)manager.protocolConfiguration;
    } else {
        proto = [[NETunnelProviderProtocol alloc] init];
        changed = YES;
    }
    changed = changed
        || ![proto.providerBundleIdentifier isEqualToString:VoyaProviderBundleIdentifier]
        || ![proto.serverAddress isEqualToString:VoyaLocalizedDescription]
        || ![manager.localizedDescription isEqualToString:VoyaLocalizedDescription];

    proto.providerBundleIdentifier = VoyaProviderBundleIdentifier;
    proto.serverAddress = VoyaLocalizedDescription;

    manager.localizedDescription = VoyaLocalizedDescription;
    manager.protocolConfiguration = proto;
    return changed;
}

/// `outNeedsSave` is set when the returned configuration differs from what the
/// system preferences hold: a new one, or one this had to correct.
static NETunnelProviderManager *VoyaLoadManager(BOOL createIfMissing, BOOL *outNeedsSave, NSError **outError) {
    if (outNeedsSave != NULL) {
        *outNeedsSave = NO;
    }
    NSError *loadError = nil;
    NSArray<NETunnelProviderManager *> *managers = VoyaLoadAllManagers(&loadError);
    if (loadError != nil) {
        if (outError != NULL) {
            *outError = loadError;
        }
        return nil;
    }

    for (NETunnelProviderManager *manager in managers) {
        NETunnelProviderProtocol *proto = nil;
        if ([manager.protocolConfiguration isKindOfClass:[NETunnelProviderProtocol class]]) {
            proto = (NETunnelProviderProtocol *)manager.protocolConfiguration;
        }
        if ([proto.providerBundleIdentifier isEqualToString:VoyaProviderBundleIdentifier]) {
            BOOL changed = VoyaConfigureManager(manager);
            if (outNeedsSave != NULL) {
                *outNeedsSave = changed;
            }
            return manager;
        }
    }

    if (!createIfMissing) {
        return nil;
    }

    NETunnelProviderManager *manager = [[NETunnelProviderManager alloc] init];
    VoyaConfigureManager(manager);
    if (outNeedsSave != NULL) {
        *outNeedsSave = YES;
    }
    return manager;
}

// The installed configuration, kept between calls. The Rust side asks for the
// status every few seconds; reading the preferences each time was a round trip
// to `nesessionmanager` per poll, and a single one that failed or timed out
// reported an error for a tunnel that was carrying traffic. The iOS host
// (`SystemTunnelHost.swift`) keeps its manager for the same reason.
static NETunnelProviderManager *VoyaKnownManager = nil;
// `YES` once `VoyaKnownManager` holds what the preferences said, `nil` included.
static BOOL VoyaKnownManagerCurrent = NO;
// Bumped by every configuration change, so a load that was in flight when one
// arrived does not mark what it read before it as current.
static NSUInteger VoyaManagerGeneration = 0;

static NSObject *VoyaManagerLock(void) {
    static NSObject *lock = nil;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        lock = [[NSObject alloc] init];
        // Never removed: the bridge lives as long as the process.
        [[NSNotificationCenter defaultCenter]
            addObserverForName:NEVPNConfigurationChangeNotification object:nil queue:nil
            usingBlock:^(NSNotification *notification) {
                (void)notification;
                @synchronized (lock) {
                    VoyaManagerGeneration += 1;
                    // The object stays: it is the answer of last resort when
                    // the read that would replace it fails.
                    VoyaKnownManagerCurrent = NO;
                }
            }];
    });
    return lock;
}

static void VoyaRememberManager(NETunnelProviderManager *manager) {
    @synchronized (VoyaManagerLock()) {
        VoyaKnownManager = manager;
        VoyaKnownManagerCurrent = YES;
        // A read that began before the save holds what the preferences said
        // then — nothing at all, on a first connect. The notification that
        // would outdate it is still on its way, so it is outdated here.
        VoyaManagerGeneration += 1;
    }
}

/// The installed configuration for status, stop and diagnostics: the kept one
/// while nothing changed, a fresh read otherwise, and the last known one when
/// that read fails. `outError` is set only when there is nothing to fall back on.
static NETunnelProviderManager *VoyaCurrentManager(NSError **outError) {
    NSUInteger generation = 0;
    @synchronized (VoyaManagerLock()) {
        if (VoyaKnownManagerCurrent) {
            return VoyaKnownManager;
        }
        generation = VoyaManagerGeneration;
    }

    NSError *loadError = nil;
    NETunnelProviderManager *manager = VoyaLoadManager(NO, NULL, &loadError);
    @synchronized (VoyaManagerLock()) {
        if (loadError != nil) {
            if (VoyaKnownManager == nil && outError != NULL) {
                *outError = loadError;
            }
            return VoyaKnownManager;
        }
        if (generation != VoyaManagerGeneration && VoyaKnownManagerCurrent) {
            // Saved while this was being read: the saved one is the answer,
            // and what was read is older than it.
            return VoyaKnownManager;
        }
        VoyaKnownManager = manager;
        VoyaKnownManagerCurrent = generation == VoyaManagerGeneration;
        return manager;
    }
}

static BOOL VoyaSaveManager(NETunnelProviderManager *manager, NSError **outError) {
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block NSError *saveError = nil;

    [manager saveToPreferencesWithCompletionHandler:^(NSError *error) {
        saveError = error;
        dispatch_semaphore_signal(semaphore);
    }];
    if (!VoyaWaitWithTimeout(semaphore, VoyaPreferencesTimeoutSeconds)) {
        if (outError != NULL) {
            *outError = VoyaMakeError(@"Timed out saving the VoyaVPN VPN configuration to system preferences.");
        }
        return NO;
    }

    if (saveError != nil && outError != NULL) {
        *outError = saveError;
    }
    return saveError == nil;
}

static BOOL VoyaReloadManager(NETunnelProviderManager *manager, NSError **outError) {
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block NSError *loadError = nil;

    [manager loadFromPreferencesWithCompletionHandler:^(NSError *error) {
        loadError = error;
        dispatch_semaphore_signal(semaphore);
    }];
    if (!VoyaWaitWithTimeout(semaphore, VoyaPreferencesTimeoutSeconds)) {
        if (outError != NULL) {
            *outError = VoyaMakeError(@"Timed out reloading the VoyaVPN VPN configuration from system preferences.");
        }
        return NO;
    }

    if (loadError != nil && outError != NULL) {
        *outError = loadError;
    }
    return loadError == nil;
}

static NSString *VoyaFetchLastDisconnectError(NETunnelProviderSession *session) {
    // A selector check rather than `@available` (macOS 13): that one compiles to
    // a call to `__isPlatformVersionAtLeast`, which Rust's std exports only
    // until the release profile's fat LTO internalizes it, and the link fails.
    if (![session respondsToSelector:@selector(fetchLastDisconnectErrorWithCompletionHandler:)]) {
        return @"";
    }

    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block NSError *disconnectError = nil;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
    [session fetchLastDisconnectErrorWithCompletionHandler:^(NSError *error) {
        disconnectError = error;
        dispatch_semaphore_signal(semaphore);
    }];
#pragma clang diagnostic pop
    // Diagnostics only: a wedged daemon must not hold up the status query
    // that asked for this context, so it gets less time than a preferences
    // read the caller cannot do without.
    if (!VoyaWaitWithTimeout(semaphore, VoyaLastDisconnectErrorTimeoutSeconds)) {
        return @"";
    }
    return disconnectError.localizedDescription ?: @"";
}

// Observe only this session. Notifications wake the waiter; bounded polling
// also handles daemon updates that arrive without a notification.
static VoyaTunnelWaitResult VoyaWaitForSession(NEVPNConnection *connection, BOOL starting, BOOL wasActive, NSTimeInterval timeout) {
    dispatch_semaphore_t changed = dispatch_semaphore_create(0);
    id observer = [[NSNotificationCenter defaultCenter]
        addObserverForName:NEVPNStatusDidChangeNotification object:connection queue:nil
        usingBlock:^(NSNotification *notification) {
            (void)notification;
            dispatch_semaphore_signal(changed);
        }];
    VoyaTunnelWaitResult result = VoyaAwaitTunnel(starting, wasActive, timeout,
        ^NEVPNStatus { return connection.status; },
        ^NSTimeInterval { return NSProcessInfo.processInfo.systemUptime; },
        ^{
            if (!starting && connection.status != NEVPNStatusDisconnected && connection.status != NEVPNStatusInvalid) {
                [connection stopVPNTunnel];
            }
            if ([NSThread isMainThread]) {
                [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode
                    beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
            } else {
                dispatch_semaphore_wait(changed, dispatch_time(DISPATCH_TIME_NOW, 200 * NSEC_PER_MSEC));
            }
        });
    [[NSNotificationCenter defaultCenter] removeObserver:observer];
    return result;
}

// Whether the session this process last started is known to have come up. Its
// start request has then been consumed, so a later stop that finds it already
// down — the provider exited, or another VPN took over — need not sit out the
// window kept for a start still queued in nesessionmanager. A start that
// failed or timed out leaves it clear, and so does a tunnel this process did
// not start: both get the whole window.
static atomic_bool VoyaStartedSessionCameUp = false;

static BOOL VoyaWaitForDisconnected(NEVPNConnection *connection, BOOL wasActive, NSTimeInterval timeoutSeconds) {
    return VoyaWaitForSession(connection, NO, wasActive, timeoutSeconds) == VoyaTunnelDisconnected;
}

static char *VoyaWaitForConnected(NETunnelProviderSession *session, int64_t timeoutMs) {
    VoyaTunnelWaitResult result = VoyaWaitForSession(session, YES, NO,
        (NSTimeInterval)(timeoutMs > 0 ? timeoutMs : 20000) / 1000.0);
    if (result == VoyaTunnelReady) {
        atomic_store(&VoyaStartedSessionCameUp, true);
        return VoyaCopyCString(@"ok");
    }

    NSString *fallback = result == VoyaTunnelTimedOut
        ? @"Timed out waiting for VoyaVPN PacketTunnel to connect."
        : result == VoyaTunnelInvalid
            ? @"VoyaVPN PacketTunnel became invalid before it became ready."
            : @"VoyaVPN PacketTunnel disconnected before it became ready.";
    // Disconnected here means the provider came up and went down again, which
    // is how every rejected config ends: the user is waiting for that error.
    // A session that timed out or went invalid has not disconnected, and the
    // last disconnect on record is an earlier session's.
    NSString *lastError = result == VoyaTunnelDisconnected ? VoyaFetchLastDisconnectError(session) : @"";
    [session stopVPNTunnel];
    BOOL stopped = VoyaWaitForDisconnected(session, result == VoyaTunnelDisconnected, 10.0);
    NSDictionary *failure = @{
        @"error": lastError.length > 0 ? lastError : fallback,
        @"cleanupError": stopped ? [NSNull null] : @"Timed out stopping VoyaVPN PacketTunnel; retry disconnect."
    };
    NSData *data = [NSJSONSerialization dataWithJSONObject:failure options:0 error:nil];
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return VoyaCopyCString([@"startFailed:" stringByAppendingString:json]);
}

char *voya_macos_packet_tunnel_status(void) {
    @autoreleasepool {
        NSError *error = nil;
        NETunnelProviderManager *manager = VoyaCurrentManager(&error);
        if (error != nil) {
            return VoyaCopyError(error);
        }
        if (manager == nil) {
            return VoyaCopyCString(@"permissionRequired");
        }

        switch (manager.connection.status) {
            case NEVPNStatusConnected:
                return VoyaCopyCString(@"running");
            case NEVPNStatusConnecting:
            case NEVPNStatusReasserting:
            case NEVPNStatusDisconnecting:
                return VoyaCopyCString(@"starting");
            case NEVPNStatusDisconnected:
            case NEVPNStatusInvalid:
                return VoyaCopyCString(@"stopped");
        }
        return VoyaCopyCString(@"error:unknown macOS PacketTunnel status");
    }
}

// `runtime_config_json` is the handshake the provider is started with, built by
// `voya_platform::tun::handoff`: it travels in the start options and is never
// written to disk, so a start from System Settings has nothing to start with.
char *voya_macos_packet_tunnel_start(const char *runtime_config_json, int64_t timeout_ms, int32_t include_all_networks) {
    @autoreleasepool {
        if (runtime_config_json == NULL) {
            return VoyaCopyCString(@"error:missing runtime config");
        }
        NSData *runtimeConfigData = [NSData dataWithBytes:runtime_config_json
                                                  length:strlen(runtime_config_json)];

        NSError *error = nil;
        NSString *activationResult = VoyaEnsureSystemExtensionActivated();
        if (activationResult.length > 0) {
            return VoyaCopyCString(activationResult);
        }

        BOOL needsSave = NO;
        NETunnelProviderManager *manager = VoyaLoadManager(YES, &needsSave, &error);
        if (error != nil) {
            return VoyaCopyError(error);
        }
        if (manager == nil) {
            return VoyaCopyCString(@"error:VoyaVPN PacketTunnel manager is unavailable.");
        }

        // Kill switch: send every network through the tunnel so nothing leaves
        // outside the VPN, while the local network stays reachable.
        if ([manager.protocolConfiguration isKindOfClass:[NETunnelProviderProtocol class]]) {
            NETunnelProviderProtocol *proto = (NETunnelProviderProtocol *)manager.protocolConfiguration;
            BOOL includeAllNetworks = include_all_networks != 0;
            needsSave = needsSave || proto.includeAllNetworks != includeAllNetworks || !proto.excludeLocalNetworks;
            proto.includeAllNetworks = includeAllNetworks;
            proto.excludeLocalNetworks = YES;
        }

        needsSave = needsSave || !manager.enabled;
        manager.enabled = YES;
        // Every connect after the first finds the configuration as it left it.
        // Saving it again costs two round trips to the system and announces a
        // change, which makes the next status read a third.
        if (needsSave) {
            if (!VoyaSaveManager(manager, &error)) {
                return VoyaCopyError(error);
            }
            if (!VoyaReloadManager(manager, &error)) {
                return VoyaCopyError(error);
            }
        }
        // A save announces a configuration change; either way this is the
        // object that holds the current one, so the next status read needs no
        // round trip.
        VoyaRememberManager(manager);

        if (![manager.connection isKindOfClass:[NETunnelProviderSession class]]) {
            return VoyaCopyCString(@"error:VoyaVPN PacketTunnel session is unavailable.");
        }
        NETunnelProviderSession *session = (NETunnelProviderSession *)manager.connection;
        NSDictionary<NSString *, NSObject *> *options = @{@"runtimeConfigJson": runtimeConfigData};
        atomic_store(&VoyaStartedSessionCameUp, false);
        if (![session startTunnelWithOptions:options andReturnError:&error]) {
            return VoyaCopyError(error);
        }
        return VoyaWaitForConnected(session, timeout_ms);
    }
}

char *voya_macos_packet_tunnel_stop(void) {
    @autoreleasepool {
        NSError *error = nil;
        NETunnelProviderManager *manager = VoyaCurrentManager(&error);
        if (error != nil) {
            return VoyaCopyError(error);
        }
        if (manager != nil) {
            [manager.connection stopVPNTunnel];
            BOOL cameUp = atomic_load(&VoyaStartedSessionCameUp);
            if (!VoyaWaitForDisconnected(manager.connection, cameUp, 10.0)) {
                return VoyaCopyCString(@"error:Timed out stopping VoyaVPN PacketTunnel; retry disconnect.");
            }
            atomic_store(&VoyaStartedSessionCameUp, false);
        }
        return VoyaCopyCString(@"ok");
    }
}

char *voya_macos_packet_tunnel_last_error(void) {
    @autoreleasepool {
        NSError *error = nil;
        NETunnelProviderManager *manager = VoyaCurrentManager(&error);
        if (error != nil) {
            return VoyaCopyError(error);
        }
        if (manager == nil || ![manager.connection isKindOfClass:[NETunnelProviderSession class]]) {
            return VoyaCopyCString(@"");
        }
        return VoyaCopyCString(VoyaFetchLastDisconnectError((NETunnelProviderSession *)manager.connection));
    }
}

char *voya_macos_packet_tunnel_container_path(void) {
    @autoreleasepool {
        NSURL *container = [[NSFileManager defaultManager]
            containerURLForSecurityApplicationGroupIdentifier:VoyaAppGroupIdentifier];
        if (container == nil) {
            return VoyaCopyCString(@"error:VoyaVPN App Group container is unavailable.");
        }
        return VoyaCopyCString(container.path ?: @"");
    }
}

void voya_macos_packet_tunnel_free(char *value) {
    free(value);
}
