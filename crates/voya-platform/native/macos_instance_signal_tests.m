#import <Foundation/Foundation.h>
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <unistd.h>

void voya_macos_post_signal(const char *name);
bool voya_macos_observe_signal(const char *name, void (*callback)(void));

static int received = 0;
static void on_signal(void) { received++; }

int main(void) {
  @autoreleasepool {
    // Unique per run, so a concurrent run cannot post into this one.
    char name[96];
    snprintf(name, sizeof name, "app.voyavpn.desktop.tests.signal.%d", getpid());
    assert(!voya_macos_observe_signal(NULL, on_signal));
    assert(!voya_macos_observe_signal(name, NULL));
    assert(voya_macos_observe_signal(name, on_signal));
    voya_macos_post_signal(NULL);
    voya_macos_post_signal(name);
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while (received == 0 && deadline.timeIntervalSinceNow > 0) {
      [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    }
    assert(received == 1 && "The posted signal never reached its observer");
  }
  puts("Instance signal bridge delivers a posted signal once.");
  return 0;
}
