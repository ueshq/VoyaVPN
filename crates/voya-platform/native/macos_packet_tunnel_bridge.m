#import <Foundation/Foundation.h>
#import <NetworkExtension/NetworkExtension.h>
#import <SystemExtensions/SystemExtensions.h>
#import <dispatch/dispatch.h>
#import <stdint.h>
#import <stdlib.h>
#import <string.h>

static NSString *const VoyaAppGroupIdentifier = @"group.app.voyavpn.desktop";
static NSString *const VoyaProviderBundleIdentifier = @"app.voyavpn.desktop.PacketTunnel";
static NSString *const VoyaRuntimeConfigRelativePath = @"Library/Application Support/VoyaVPN/packet-tunnel-runtime.json";
static NSString *const VoyaProviderStatusRelativePath = @"Library/Application Support/VoyaVPN/packet-tunnel-status.json";
static NSString *const VoyaProviderLogRelativePath = @"Library/Application Support/VoyaVPN/provider.log";
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

static void VoyaWait(dispatch_semaphore_t semaphore) {
    if (![NSThread isMainThread]) {
        dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);
        return;
    }

    while (dispatch_semaphore_wait(semaphore, DISPATCH_TIME_NOW) != 0) {
        @autoreleasepool {
            NSDate *limit = [NSDate dateWithTimeIntervalSinceNow:0.05];
            [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:limit];
        }
    }
}

static BOOL VoyaWaitWithTimeout(dispatch_semaphore_t semaphore, NSTimeInterval timeoutSeconds) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:timeoutSeconds];
    while ([[NSDate date] compare:deadline] == NSOrderedAscending) {
        if (dispatch_semaphore_wait(semaphore, DISPATCH_TIME_NOW) == 0) {
            return YES;
        }
        if ([NSThread isMainThread]) {
            NSDate *limit = [NSDate dateWithTimeIntervalSinceNow:0.05];
            [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:limit];
        } else {
            [NSThread sleepForTimeInterval:0.05];
        }
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
    VoyaWait(semaphore);

    if (loadedError != nil && outError != NULL) {
        *outError = loadedError;
    }
    return loadedError == nil ? loadedManagers : nil;
}

static void VoyaConfigureManager(NETunnelProviderManager *manager) {
    NETunnelProviderProtocol *proto = nil;
    if ([manager.protocolConfiguration isKindOfClass:[NETunnelProviderProtocol class]]) {
        proto = (NETunnelProviderProtocol *)manager.protocolConfiguration;
    } else {
        proto = [[NETunnelProviderProtocol alloc] init];
    }

    proto.providerBundleIdentifier = VoyaProviderBundleIdentifier;
    proto.serverAddress = VoyaLocalizedDescription;
    proto.providerConfiguration = @{
        @"runtimeConfigRelativePath": VoyaRuntimeConfigRelativePath,
        @"appGroupIdentifier": VoyaAppGroupIdentifier,
    };

    manager.localizedDescription = VoyaLocalizedDescription;
    manager.protocolConfiguration = proto;
}

static NETunnelProviderManager *VoyaLoadManager(BOOL createIfMissing, NSError **outError) {
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
            VoyaConfigureManager(manager);
            return manager;
        }
    }

    if (!createIfMissing) {
        return nil;
    }

    NETunnelProviderManager *manager = [[NETunnelProviderManager alloc] init];
    VoyaConfigureManager(manager);
    return manager;
}

static BOOL VoyaSaveManager(NETunnelProviderManager *manager, NSError **outError) {
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block NSError *saveError = nil;

    [manager saveToPreferencesWithCompletionHandler:^(NSError *error) {
        saveError = error;
        dispatch_semaphore_signal(semaphore);
    }];
    VoyaWait(semaphore);

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
    VoyaWait(semaphore);

    if (loadError != nil && outError != NULL) {
        *outError = loadError;
    }
    return loadError == nil;
}

static NSString *VoyaFetchLastDisconnectError(NETunnelProviderSession *session) {
    if (@available(macOS 13.0, *)) {
        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
        __block NSError *disconnectError = nil;
        [session fetchLastDisconnectErrorWithCompletionHandler:^(NSError *error) {
            disconnectError = error;
            dispatch_semaphore_signal(semaphore);
        }];
        VoyaWait(semaphore);
        return disconnectError.localizedDescription ?: @"";
    }

    return @"";
}

static char *VoyaCopySessionTerminalError(NETunnelProviderSession *session, NSString *fallback) {
    NSString *lastError = VoyaFetchLastDisconnectError(session);
    if (lastError.length > 0) {
        return VoyaCopyCString([@"error:" stringByAppendingString:lastError]);
    }
    return VoyaCopyCString([@"error:" stringByAppendingString:fallback]);
}

/// Waits for the session to leave the disconnecting state so a restart does not
/// hand `startTunnelWithOptions:` a session macOS is still tearing down. Best
/// effort: a timeout is reported by the caller's next status query rather than
/// turning a disconnect into a failure.
static void VoyaWaitForDisconnected(NEVPNConnection *connection, NSTimeInterval timeoutSeconds) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:timeoutSeconds];

    while ([[NSDate date] compare:deadline] == NSOrderedAscending) {
        if (connection.status == NEVPNStatusDisconnected || connection.status == NEVPNStatusInvalid) {
            return;
        }
        if ([NSThread isMainThread]) {
            NSDate *limit = [NSDate dateWithTimeIntervalSinceNow:0.1];
            [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:limit];
        } else {
            [NSThread sleepForTimeInterval:0.1];
        }
    }
}

static char *VoyaWaitForConnected(NETunnelProviderSession *session, int64_t timeoutMs) {
    int64_t effectiveTimeoutMs = timeoutMs > 0 ? timeoutMs : 20000;
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:(NSTimeInterval)effectiveTimeoutMs / 1000.0];

    while ([[NSDate date] compare:deadline] == NSOrderedAscending) {
        switch (session.status) {
            case NEVPNStatusConnected:
                return VoyaCopyCString(@"ok");
            case NEVPNStatusDisconnected:
                return VoyaCopySessionTerminalError(session, @"VoyaVPN PacketTunnel disconnected before it became ready.");
            case NEVPNStatusInvalid:
                return VoyaCopySessionTerminalError(session, @"VoyaVPN PacketTunnel became invalid before it became ready.");
            case NEVPNStatusConnecting:
            case NEVPNStatusReasserting:
            case NEVPNStatusDisconnecting:
                break;
        }

        if ([NSThread isMainThread]) {
            NSDate *limit = [NSDate dateWithTimeIntervalSinceNow:0.2];
            [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:limit];
        } else {
            [NSThread sleepForTimeInterval:0.2];
        }
    }

    [session stopVPNTunnel];
    return VoyaCopyCString(@"error:Timed out waiting for VoyaVPN PacketTunnel to connect.");
}

static NSURL *VoyaAppGroupURL(NSString *relativePath, NSError **outError) {
    NSURL *container = [[NSFileManager defaultManager]
        containerURLForSecurityApplicationGroupIdentifier:VoyaAppGroupIdentifier];
    if (container == nil) {
        if (outError != NULL) {
            *outError = VoyaMakeError(@"VoyaVPN App Group container is unavailable.");
        }
        return nil;
    }
    return [container URLByAppendingPathComponent:relativePath];
}

static NSURL *VoyaRuntimeConfigURL(NSError **outError) {
    return VoyaAppGroupURL(VoyaRuntimeConfigRelativePath, outError);
}

static NSData *VoyaCreateRuntimeConfigData(NSString *configPath, NSString *profileId, NSError **outError) {
    if (![configPath hasPrefix:@"/"]) {
        if (outError != NULL) {
            *outError = VoyaMakeError(@"config path must be absolute");
        }
        return nil;
    }

    NSError *readError = nil;
    NSString *singboxConfigJson = [NSString stringWithContentsOfFile:configPath
                                                           encoding:NSUTF8StringEncoding
                                                              error:&readError];
    if (readError != nil) {
        if (outError != NULL) {
            *outError = readError;
        }
        return nil;
    }

    NSURL *statusURL = VoyaAppGroupURL(VoyaProviderStatusRelativePath, outError);
    if (statusURL == nil) {
        return nil;
    }
    NSURL *logURL = VoyaAppGroupURL(VoyaProviderLogRelativePath, outError);
    if (logURL == nil) {
        return nil;
    }

    NSDictionary *runtimeConfig = @{
        @"version": @1,
        @"activeProfileId": profileId ?: [NSNull null],
        @"mainConfigPath": configPath,
        @"statusPath": statusURL.path ?: @"",
        @"logPath": logURL.path ?: @"",
        @"singboxConfigJson": singboxConfigJson ?: @"",
    };

    NSError *encodeError = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:runtimeConfig options:0 error:&encodeError];
    if (encodeError != nil) {
        if (outError != NULL) {
            *outError = encodeError;
        }
        return nil;
    }
    return data;
}

static BOOL VoyaWriteRuntimeConfigData(NSData *data, NSError **outError) {
    NSURL *destination = VoyaRuntimeConfigURL(outError);
    if (destination == nil) {
        return NO;
    }

    NSError *mkdirError = nil;
    BOOL created = [[NSFileManager defaultManager] createDirectoryAtURL:destination.URLByDeletingLastPathComponent
                                            withIntermediateDirectories:YES
                                                             attributes:nil
                                                                  error:&mkdirError];
    if (!created) {
        if (outError != NULL) {
            *outError = mkdirError;
        }
        return NO;
    }

    NSError *writeError = nil;
    BOOL written = [data writeToURL:destination options:NSDataWritingAtomic error:&writeError];
    if (!written && outError != NULL) {
        *outError = writeError;
    }
    return written;
}

char *voya_macos_packet_tunnel_status(void) {
    @autoreleasepool {
        NSError *error = nil;
        NETunnelProviderManager *manager = VoyaLoadManager(NO, &error);
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

char *voya_macos_packet_tunnel_start(const char *config_path, const char *profile_id, int64_t timeout_ms) {
    @autoreleasepool {
        if (config_path == NULL) {
            return VoyaCopyCString(@"error:missing config path");
        }

        NSString *configPath = [NSString stringWithUTF8String:config_path];
        NSString *profileId = profile_id != NULL ? [NSString stringWithUTF8String:profile_id] : nil;
        if (configPath == nil) {
            return VoyaCopyCString(@"error:config path is not valid UTF-8");
        }
        if (profile_id != NULL && profileId == nil) {
            return VoyaCopyCString(@"error:profile id is not valid UTF-8");
        }

        NSError *error = nil;
        NSData *runtimeConfigData = VoyaCreateRuntimeConfigData(configPath, profileId, &error);
        if (runtimeConfigData == nil) {
            return VoyaCopyError(error);
        }
        if (!VoyaWriteRuntimeConfigData(runtimeConfigData, &error)) {
            return VoyaCopyError(error);
        }

        NSString *activationResult = VoyaEnsureSystemExtensionActivated();
        if (activationResult.length > 0) {
            return VoyaCopyCString(activationResult);
        }

        NETunnelProviderManager *manager = VoyaLoadManager(YES, &error);
        if (error != nil) {
            return VoyaCopyError(error);
        }
        if (manager == nil) {
            return VoyaCopyCString(@"error:VoyaVPN PacketTunnel manager is unavailable.");
        }

        manager.enabled = YES;
        if (!VoyaSaveManager(manager, &error)) {
            return VoyaCopyError(error);
        }
        if (!VoyaReloadManager(manager, &error)) {
            return VoyaCopyError(error);
        }

        if (![manager.connection isKindOfClass:[NETunnelProviderSession class]]) {
            return VoyaCopyCString(@"error:VoyaVPN PacketTunnel session is unavailable.");
        }
        NETunnelProviderSession *session = (NETunnelProviderSession *)manager.connection;
        NSDictionary<NSString *, NSObject *> *options = @{@"runtimeConfigJson": runtimeConfigData};
        if (![session startTunnelWithOptions:options andReturnError:&error]) {
            return VoyaCopyError(error);
        }
        return VoyaWaitForConnected(session, timeout_ms);
    }
}

char *voya_macos_packet_tunnel_stop(void) {
    @autoreleasepool {
        NSError *error = nil;
        NETunnelProviderManager *manager = VoyaLoadManager(NO, &error);
        if (error != nil) {
            return VoyaCopyError(error);
        }
        if (manager != nil) {
            [manager.connection stopVPNTunnel];
            VoyaWaitForDisconnected(manager.connection, 10.0);
        }
        return VoyaCopyCString(@"ok");
    }
}

char *voya_macos_packet_tunnel_last_error(void) {
    @autoreleasepool {
        NSError *error = nil;
        NETunnelProviderManager *manager = VoyaLoadManager(NO, &error);
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
