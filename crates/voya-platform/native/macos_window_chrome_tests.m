#import <AppKit/AppKit.h>
#include <assert.h>
#include <math.h>

void voya_install_window_chrome(void *handle, double left, double top);

static void finish_layout(void) {
  // The inset is applied asynchronously on the main queue. A fixed delay can
  // expire before AppKit services that queue, especially during a build.
  __block BOOL finished = NO;
  dispatch_async(dispatch_get_main_queue(), ^{ finished = YES; });
  NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
  while (!finished && deadline.timeIntervalSinceNow > 0) {
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
  }
  assert(finished && "Timed out waiting for the main queue to finish window layout");
}

static void assert_inset(NSWindow *window) {
  NSArray<NSButton *> *buttons = @[
    [window standardWindowButton:NSWindowCloseButton],
    [window standardWindowButton:NSWindowMiniaturizeButton],
    [window standardWindowButton:NSWindowZoomButton]
  ];
  CGFloat previous = 0;
  for (NSButton *button in buttons) {
    NSRect rect = [button convertRect:button.bounds toView:nil];
    assert(fabs(NSHeight(window.frame) - NSMaxY(rect) - 32) < 0.1);
    assert(NSMinX(rect) > previous);
    previous = NSMinX(rect);
    assert(!button.hidden);
  }
  NSRect close = [buttons[0] convertRect:buttons[0].bounds toView:nil];
  assert(fabs(NSMinX(close) - 20) < 0.1);
}

int main(void) {
  @autoreleasepool {
    [NSApplication sharedApplication];
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 1180, 760)
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                  NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable |
                  NSWindowStyleMaskFullSizeContentView
        backing:NSBackingStoreBuffered defer:NO];
    window.releasedWhenClosed = NO;
    window.titleVisibility = NSWindowTitleHidden;
    window.titlebarAppearsTransparent = YES;
    NSButton *close = [window standardWindowButton:NSWindowCloseButton];
    id target = close.target;
    SEL action = close.action;
    voya_install_window_chrome((__bridge void *)window, 20, 32);
    finish_layout();
    assert_inset(window);
    assert(close.target == target && close.action == action);

    // Reinstalling must reuse the window-owned observer, and resizing or AppKit
    // restoring its default button positions must not lose the requested inset.
    voya_install_window_chrome((__bridge void *)window, 20, 32);
    [window setContentSize:NSMakeSize(960, 640)];
    finish_layout();
    assert_inset(window);
    for (NSNotificationName name in @[NSWindowDidBecomeKeyNotification,
                                     NSWindowDidMoveNotification,
                                     NSWindowDidUpdateNotification,
                                     NSWindowDidExitFullScreenNotification,
                                     NSWindowDidDeminiaturizeNotification]) {
      [close setFrameOrigin:NSMakePoint(8, 8)];
      [NSNotificationCenter.defaultCenter postNotificationName:name object:window];
      finish_layout();
      assert_inset(window);
    }
    [window close];
  }
  puts("Native window chrome checks passed.");
  return 0;
}
