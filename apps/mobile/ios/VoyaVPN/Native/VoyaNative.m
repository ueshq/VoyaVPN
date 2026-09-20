#import <React/RCTBridgeModule.h>
#import <React/RCTEventEmitter.h>

/**
 * The React Native registration for the Swift `VoyaNative`.
 *
 * A Swift module still needs its methods declared to the bridge in Objective-C:
 * `@objc` exposes them to the runtime, and these macros are what tells React
 * Native they exist and what their JS signature is.
 */
@interface RCT_EXTERN_MODULE (VoyaNative, RCTEventEmitter)

RCT_EXTERN_METHOD(invoke
                  : (NSString *)command argsJson
                  : (NSString *)argsJson resolve
                  : (RCTPromiseResolveBlock)resolve reject
                  : (RCTPromiseRejectBlock)reject)

@end
