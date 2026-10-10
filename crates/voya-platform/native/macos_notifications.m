#import <Foundation/Foundation.h>
#import <UserNotifications/UserNotifications.h>
#include <stdbool.h>

// The app notifies only while its window is hidden, but hiding the window
// leaves the app active, and the system shows nothing for the active app
// unless its delegate asks for it.
@interface VoyaNotificationPresenter : NSObject <UNUserNotificationCenterDelegate>
@end

@implementation VoyaNotificationPresenter
- (void)userNotificationCenter:(UNUserNotificationCenter *)center
       willPresentNotification:(UNNotification *)notification
         withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completionHandler {
  (void)center;
  (void)notification;
  if (@available(macOS 11.0, *)) {
    completionHandler(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionList);
  } else {
    completionHandler(UNNotificationPresentationOptionAlert);
  }
}
@end

static UNUserNotificationCenter *notification_center(void) {
  // Outside an app bundle the framework raises instead of returning nil, and
  // a `tauri dev` binary is exactly that.
  if (!NSBundle.mainBundle.bundleIdentifier) return nil;
  UNUserNotificationCenter *center = UNUserNotificationCenter.currentNotificationCenter;
  static VoyaNotificationPresenter *presenter;
  static dispatch_once_t once;
  dispatch_once(&once, ^{
    presenter = [VoyaNotificationPresenter new];
    center.delegate = presenter;
  });
  return center;
}

// Posts a notification, asking the user to allow them the first time: the
// system shows that prompt once and answers from its record afterwards.
// Returns false when nothing could be asked of the system at all; a refusal
// is the user's answer and is not reported.
bool voya_macos_post_notification(const char *title, const char *body) {
  NSString *titleText = title ? [NSString stringWithUTF8String:title] : nil;
  if (!titleText) return false;
  UNUserNotificationCenter *center = notification_center();
  if (!center) return false;

  UNMutableNotificationContent *content = [UNMutableNotificationContent new];
  content.title = titleText;
  NSString *bodyText = body ? [NSString stringWithUTF8String:body] : nil;
  if (bodyText) content.body = bodyText;

  [center requestAuthorizationWithOptions:UNAuthorizationOptionAlert
                        completionHandler:^(BOOL granted, NSError *error) {
    (void)error;
    if (!granted) return;
    UNNotificationRequest *request =
        [UNNotificationRequest requestWithIdentifier:NSUUID.UUID.UUIDString content:content trigger:nil];
    [center addNotificationRequest:request withCompletionHandler:^(NSError *postError) {
      if (postError) NSLog(@"VoyaVPN could not post a notification: %@", postError);
    }];
  }];
  return true;
}
