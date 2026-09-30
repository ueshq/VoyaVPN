// Launch at login through SMAppService. See macos_login_item.m.
#ifndef VOYA_MACOS_LOGIN_ITEM_H
#define VOYA_MACOS_LOGIN_ITEM_H

#include <stdint.h>

// 0...3 mirror SMAppServiceStatus; the negative values are bridge outcomes.
enum {
    VoyaLoginItemNotRegistered = 0,
    VoyaLoginItemEnabled = 1,
    VoyaLoginItemRequiresApproval = 2,
    VoyaLoginItemNotFound = 3,
    // *error holds a message.
    VoyaLoginItemFailed = -1,
    // macOS before 13, or the process is not running from an .app bundle;
    // *error says which.
    VoyaLoginItemUnavailable = -2,
};

// Each call sets *error to NULL or to a strdup'd message that the caller
// releases with voya_macos_login_item_free. plist_name is borrowed for the
// duration of the call.
int32_t voya_macos_login_item_status(const char *plist_name, char **error);
// Returns the status after registering.
int32_t voya_macos_login_item_register(const char *plist_name, char **error);
// Returns the status after unregistering.
int32_t voya_macos_login_item_unregister(const char *plist_name, char **error);
void voya_macos_login_item_free(char *value);

#endif
