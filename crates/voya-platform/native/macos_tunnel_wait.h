#import <Foundation/Foundation.h>
#import <NetworkExtension/NetworkExtension.h>

typedef NS_ENUM(NSInteger, VoyaTunnelWaitResult) {
    VoyaTunnelReady,
    VoyaTunnelDisconnected,
    VoyaTunnelInvalid,
    VoyaTunnelTimedOut,
};

// The production bridge and the native test harness use the same waiter.
// Time and status are injected; no VPN profile is needed to exercise races.
//
// `wasActive` says the caller already watched this session leave the
// disconnected state — a start that came up and failed. Its start request has
// been consumed, so a stop need not sit out the whole window for one.
static inline VoyaTunnelWaitResult VoyaAwaitTunnel(
    BOOL starting, BOOL wasActive, NSTimeInterval timeout,
    NEVPNStatus (^status)(void), NSTimeInterval (^now)(void), void (^wait)(void)
) {
    NSTimeInterval deadline = now() + timeout;
    BOOL progressed = NO;
    BOOL observedActive = wasActive;
    // A stop can race a start request still queued in nesessionmanager. Require
    // a quiet disconnected interval, and keep stopping if it starts late.
    NSTimeInterval disconnectedSince = -1;
    while (now() < deadline) {
        NEVPNStatus current = status();
        if (starting) {
            if (current == NEVPNStatusConnected) return VoyaTunnelReady;
            if (current == NEVPNStatusInvalid) return VoyaTunnelInvalid;
            if (current == NEVPNStatusDisconnected && progressed) return VoyaTunnelDisconnected;
            if (current == NEVPNStatusConnecting || current == NEVPNStatusReasserting) progressed = YES;
        } else {
            if (current == NEVPNStatusDisconnected || current == NEVPNStatusInvalid) {
                if (disconnectedSince < 0) disconnectedSince = now();
                // An initially disconnected session can still have a queued
                // start. Observe the full cleanup window in that case.
                if (observedActive && now() - disconnectedSince >= 1.0) return VoyaTunnelDisconnected;
            } else {
                observedActive = YES;
                disconnectedSince = -1;
            }
        }
        wait();
    }
    if (!starting && disconnectedSince >= 0 && now() - disconnectedSince >= 1.0
        && (status() == NEVPNStatusDisconnected || status() == NEVPNStatusInvalid)) {
        return VoyaTunnelDisconnected;
    }
    return VoyaTunnelTimedOut;
}
