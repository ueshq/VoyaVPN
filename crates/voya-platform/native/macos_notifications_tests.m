#import <Foundation/Foundation.h>
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>

bool voya_macos_post_notification(const char *title, const char *body);

int main(void) {
  @autoreleasepool {
    // This test is a bare executable, as a `tauri dev` build is: the
    // notification center raises there, so the bridge must not reach it.
    assert(NSBundle.mainBundle.bundleIdentifier == nil);
    assert(!voya_macos_post_notification(NULL, NULL));
    assert(!voya_macos_post_notification("Title", NULL));
    assert(!voya_macos_post_notification("Title", "Body"));
  }
  puts("Notification bridge stays out of the notification center without an app bundle.");
  return 0;
}
