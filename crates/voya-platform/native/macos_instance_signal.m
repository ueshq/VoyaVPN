#import <Foundation/Foundation.h>
#include <notify.h>
#include <stdbool.h>

// A Darwin notification is the one signal two copies of the app can exchange
// inside the App Sandbox without a socket or file path both may open: the
// single-instance plugin's socket in /tmp is denied there.
void voya_macos_post_signal(const char *name) {
  if (name) notify_post(name);
}

// Calls `callback` on the main queue each time `name` is posted, by any
// process. `name` is copied by the system; the registration lasts for the
// life of the process.
bool voya_macos_observe_signal(const char *name, void (*callback)(void)) {
  if (!name || !callback) return false;
  int token = 0;
  return notify_register_dispatch(name, &token, dispatch_get_main_queue(), ^(int fired) {
    (void)fired;
    callback();
  }) == NOTIFY_STATUS_OK;
}
