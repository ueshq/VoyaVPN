#import <React/RCTBridgeModule.h>
@interface RCT_EXTERN_MODULE(VoyaDeviceActions, NSObject)
RCT_EXTERN_METHOD(scanQr:(NSString *)cancelLabel resolve:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)
RCT_EXTERN_METHOD(pickQr:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)
RCT_EXTERN_METHOD(shareDiagnostics:(NSString *)text resolve:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)
RCT_EXTERN_METHOD(appVersion:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject)
@end
