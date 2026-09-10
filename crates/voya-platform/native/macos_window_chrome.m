#import <AppKit/AppKit.h>
#import <objc/runtime.h>

// AppKit can reset the Tauri/Wry inset when activating, resizing or leaving
// fullscreen. Reapply after its layout pass, using window coordinates rather
// than relying on the titlebar's private view hierarchy's local coordinates.
@interface VoyaWindowChrome : NSObject
@property(nonatomic, weak) NSWindow *window;
@property(nonatomic) CGFloat left;
@property(nonatomic) CGFloat top;
@property(nonatomic) BOOL pending;
@property(nonatomic, strong) NSMutableArray *observers;
- (void)scheduleLayout;
@end

@implementation VoyaWindowChrome
- (void)scheduleLayout {
  if (self.pending) return;
  self.pending = YES;
  __weak VoyaWindowChrome *weakSelf = self;
  dispatch_async(dispatch_get_main_queue(), ^{
    VoyaWindowChrome *owner = weakSelf;
    if (!owner) return;
    owner.pending = NO;
    NSWindow *window = owner.window;
    if (!window || (window.styleMask & NSWindowStyleMaskFullScreen)) return;

    NSButton *close = [window standardWindowButton:NSWindowCloseButton];
    NSButton *minimize = [window standardWindowButton:NSWindowMiniaturizeButton];
    NSButton *zoom = [window standardWindowButton:NSWindowZoomButton];
    if (!close || !minimize || !zoom) return;
    CGFloat spacing = NSMinX(minimize.frame) - NSMinX(close.frame);
    NSView *titlebar = close.superview.superview;
    if (!titlebar) return;
    NSRect frame = titlebar.frame;
    frame.size.height = owner.top + NSHeight(close.frame);
    frame.origin.y = NSHeight(window.frame) - NSHeight(frame);
    if (!NSEqualRects(titlebar.frame, frame)) titlebar.frame = frame;

    NSArray<NSButton *> *buttons = @[close, minimize, zoom];
    for (NSUInteger index = 0; index < buttons.count; index++) {
      NSButton *button = buttons[index];
      NSRect target = NSMakeRect(owner.left + index * spacing,
                                NSHeight(window.frame) - owner.top - NSHeight(button.frame),
                                NSWidth(button.frame), NSHeight(button.frame));
      NSRect local = [button.superview convertRect:target fromView:nil];
      if (!NSEqualRects(button.frame, local)) button.frame = local;
    }
  });
}

- (void)dealloc {
  for (id observer in self.observers) {
    [NSNotificationCenter.defaultCenter removeObserver:observer];
  }
}
@end

void voya_install_window_chrome(void *handle, double left, double top) {
  NSCAssert(NSThread.isMainThread, @"Window chrome requires the main thread");
  NSWindow *window = (__bridge NSWindow *)handle;
  if (!window) return;
  static char ownerKey;
  VoyaWindowChrome *owner = objc_getAssociatedObject(window, &ownerKey);
  if (!owner) {
    owner = [VoyaWindowChrome new];
    owner.window = window;
    owner.observers = [NSMutableArray new];
    __weak VoyaWindowChrome *weakOwner = owner;
    for (NSNotificationName name in @[
      NSWindowDidResizeNotification, NSWindowDidMoveNotification, NSWindowDidUpdateNotification,
      NSWindowDidBecomeKeyNotification,
      NSWindowDidResignKeyNotification, NSWindowDidDeminiaturizeNotification,
      NSWindowDidExitFullScreenNotification, NSWindowDidChangeBackingPropertiesNotification
    ]) {
      id observer = [NSNotificationCenter.defaultCenter addObserverForName:name
          object:window queue:nil usingBlock:^(NSNotification *notification) {
            (void)notification;
            [weakOwner scheduleLayout];
          }];
      [owner.observers addObject:observer];
    }
    objc_setAssociatedObject(window, &ownerKey, owner, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
  }
  owner.left = left;
  owner.top = top;
  [owner scheduleLayout];
}
