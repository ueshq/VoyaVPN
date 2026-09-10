#import <AppKit/AppKit.h>
#import <SystemConfiguration/SystemConfiguration.h>
#import <string.h>

// Read every service, including disabled services that may be enabled later.
// Return only endpoints: never export proxy authentication credentials.
char *voya_macos_proxy_observation(void) {
    @autoreleasepool {
        SCPreferencesRef preferences = SCPreferencesCreate(NULL, CFSTR("VoyaVPN proxy inspection"), NULL);
        if (preferences == NULL) return strdup("null");
        CFArrayRef services = SCNetworkServiceCopyAll(preferences);
        CFRelease(preferences);
        if (services == NULL) return strdup("null");
        NSMutableArray *servers = [NSMutableArray array];
        NSMutableArray *pacURLs = [NSMutableArray array];
        BOOL known = YES;
        for (id item in (__bridge NSArray *)services) {
            SCNetworkProtocolRef protocol = SCNetworkServiceCopyProtocol((__bridge SCNetworkServiceRef)item, kSCNetworkProtocolTypeProxies);
            if (protocol == NULL) {
                if (SCError() != kSCStatusNoKey) known = NO;
                continue;
            }
            NSDictionary *settings = (__bridge NSDictionary *)SCNetworkProtocolGetConfiguration(protocol);
            if (settings == nil) known = NO;
            for (NSString *type in @[@"HTTP", @"HTTPS", @"SOCKS"]) {
                if ([settings[[type stringByAppendingString:@"Enable"]] boolValue]) {
                    id host = settings[[type stringByAppendingString:@"Proxy"]];
                    if ([host isKindOfClass:NSString.class] && [host length] > 0) [servers addObject:host];
                    else known = NO;
                }
            }
            if ([settings[@"ProxyAutoConfigEnable"] boolValue]) {
                id url = settings[@"ProxyAutoConfigURLString"];
                if ([url isKindOfClass:NSString.class] && [url length] > 0) [pacURLs addObject:url];
                else known = NO;
            }
            // WPAD can change proxies without an explicit endpoint to inspect.
            if ([settings[@"ProxyAutoDiscoveryEnable"] boolValue]) known = NO;
            CFRelease(protocol);
        }
        CFRelease(services);
        NSData *data = [NSJSONSerialization dataWithJSONObject:@{@"known": @(known), @"servers": servers, @"pacURLs": pacURLs} options:0 error:nil];
        return strdup([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String ?: "null");
    }
}

int voya_macos_open_network_settings(void) {
    @autoreleasepool {
        __block BOOL opened = NO;
        void (^open)(void) = ^{
            opened = [NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.Network-Settings.extension"]];
        };
        if (NSThread.isMainThread) open();
        else dispatch_sync(dispatch_get_main_queue(), open);
        return opened ? 0 : 1;
    }
}

void voya_macos_proxy_free(char *value) { free(value); }
