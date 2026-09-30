// Run by scripts/native/macos/test-bridge.mjs. The test binary is not inside
// an .app bundle, so every entry point must refuse cleanly rather than touch
// the user's real login items.
#import <Foundation/Foundation.h>
#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "macos_login_item.h"

static const char *kPlist = "app.voyavpn.desktop.autostart.plist";

static void assert_unavailable(int32_t result, char *error) {
    assert(result == VoyaLoginItemUnavailable);
    assert(error != NULL);
    assert(strstr(error, ".app bundle") != NULL);
    voya_macos_login_item_free(error);
}

int main(void) {
    @autoreleasepool {
        char *error = NULL;
        assert_unavailable(voya_macos_login_item_status(kPlist, &error), error);
        error = NULL;
        assert_unavailable(voya_macos_login_item_register(kPlist, &error), error);
        error = NULL;
        assert_unavailable(voya_macos_login_item_unregister(kPlist, &error), error);

        error = NULL;
        assert(voya_macos_login_item_status(NULL, &error) == VoyaLoginItemFailed);
        assert(error != NULL && strstr(error, "empty") != NULL);
        voya_macos_login_item_free(error);

        error = NULL;
        assert(voya_macos_login_item_register("", &error) == VoyaLoginItemFailed);
        voya_macos_login_item_free(error);

        // A NULL out-pointer is allowed, and free accepts NULL.
        assert(voya_macos_login_item_status(kPlist, NULL) == VoyaLoginItemUnavailable);
        voya_macos_login_item_free(NULL);
    }
    puts("Login item bridge refuses cleanly outside an .app bundle.");
    return 0;
}
