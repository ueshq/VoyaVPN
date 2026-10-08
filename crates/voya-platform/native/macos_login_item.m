// Launch at login through SMAppService (macOS 13+).
//
// The app registers the launchd agent bundled at
// Contents/Library/LaunchAgents/<plist_name>. That is the store-safe way to
// launch at login: nothing is written outside the signed bundle, and the user
// controls the item in System Settings > General > Login Items & Extensions.

#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>
#include <stdlib.h>
#include <string.h>

#include "macos_login_item.h"

static char *VoyaLoginItemCopy(NSString *message) {
    const char *utf8 = message.UTF8String;
    return strdup(utf8 != NULL ? utf8 : "unknown login item error");
}

static int32_t VoyaLoginItemFail(NSString *message, char **error) {
    if (error != NULL) {
        *error = VoyaLoginItemCopy(message);
    }
    return VoyaLoginItemFailed;
}

// SMAppService is macOS 13+, and the bridge is built for an older deployment
// target. `@available` would need `__isPlatformVersionAtLeast` from
// compiler-rt, which a Rust link (`-nodefaultlibs`) does not provide, so the
// class is looked up at run time instead and the availability warnings are
// silenced only for the code that runs after that lookup succeeded.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability"
#pragma clang diagnostic ignored "-Wunguarded-availability-new"

// Checks what every entry point needs, and on success returns the agent
// service through *service.
static int32_t VoyaLoginItemPrepare(const char *plist_name, char **error, SMAppService **service) {
    if (error != NULL) {
        *error = NULL;
    }
    if (plist_name == NULL || plist_name[0] == '\0') {
        return VoyaLoginItemFail(@"login item plist name is empty", error);
    }
    if (![NSBundle.mainBundle.bundlePath hasSuffix:@".app"]) {
        if (error != NULL) {
            *error = VoyaLoginItemCopy(@"VoyaVPN is not running from an .app bundle, so it has no "
                                       @"login item to register (vp run tauri dev or cargo run)");
        }
        return VoyaLoginItemUnavailable;
    }
    Class serviceClass = NSClassFromString(@"SMAppService");
    if (serviceClass == Nil) {
        if (error != NULL) {
            *error = VoyaLoginItemCopy(@"launch at login needs macOS 13 or later");
        }
        return VoyaLoginItemUnavailable;
    }
    NSString *name = [NSString stringWithUTF8String:plist_name];
    if (name == nil) {
        return VoyaLoginItemFail(@"login item plist name is not UTF-8", error);
    }
    *service = [serviceClass agentServiceWithPlistName:name];
    return VoyaLoginItemNotRegistered;
}

static int32_t VoyaLoginItemFailWithError(NSString *operation, NSError *failure, char **error) {
    return VoyaLoginItemFail(
        [NSString stringWithFormat:@"SMAppService %@ failed (%@ %ld): %@", operation,
                                   failure.domain ?: @"unknown", (long)failure.code,
                                   failure.localizedDescription ?: @"no description"],
        error);
}

int32_t voya_macos_login_item_status(const char *plist_name, char **error) {
    SMAppService *service = nil;
    int32_t prepared = VoyaLoginItemPrepare(plist_name, error, &service);
    if (prepared != VoyaLoginItemNotRegistered) {
        return prepared;
    }
    return (int32_t)service.status;
}

int32_t voya_macos_login_item_register(const char *plist_name, char **error) {
    SMAppService *service = nil;
    int32_t prepared = VoyaLoginItemPrepare(plist_name, error, &service);
    if (prepared != VoyaLoginItemNotRegistered) {
        return prepared;
    }
    SMAppServiceStatus status = service.status;
    // Registering again would fail with "already registered", and with
    // RunAtLoad it would launch a second copy of the app.
    if (status == SMAppServiceStatusEnabled || status == SMAppServiceStatusRequiresApproval) {
        return (int32_t)status;
    }
    NSError *failure = nil;
    if (![service registerAndReturnError:&failure]) {
        return VoyaLoginItemFailWithError(@"register", failure, error);
    }
    return (int32_t)service.status;
}

int32_t voya_macos_login_item_unregister(const char *plist_name, char **error) {
    SMAppService *service = nil;
    int32_t prepared = VoyaLoginItemPrepare(plist_name, error, &service);
    if (prepared != VoyaLoginItemNotRegistered) {
        return prepared;
    }
    SMAppServiceStatus status = service.status;
    if (status == SMAppServiceStatusNotRegistered || status == SMAppServiceStatusNotFound) {
        return (int32_t)status;
    }
    NSError *failure = nil;
    if (![service unregisterAndReturnError:&failure]) {
        return VoyaLoginItemFailWithError(@"unregister", failure, error);
    }
    return (int32_t)service.status;
}

#pragma clang diagnostic pop

void voya_macos_login_item_free(char *value) {
    free(value);
}
