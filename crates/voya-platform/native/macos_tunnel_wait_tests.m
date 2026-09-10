#import "macos_tunnel_wait.h"
#include <assert.h>

typedef struct {
    VoyaTunnelWaitResult result;
    double elapsed;
} WaitOutcome;

static WaitOutcome run(BOOL starting, NSArray<NSNumber *> *states, double timeout) {
    __block NSUInteger index = 0;
    __block double clock = 0;
    VoyaTunnelWaitResult result = VoyaAwaitTunnel(starting, timeout,
        ^NEVPNStatus { return (NEVPNStatus)states[MIN(index, states.count - 1)].integerValue; },
        ^double { return clock; },
        ^{ index++; clock += 0.25; });
    return (WaitOutcome){ result, clock };
}

int main(void) {
    @autoreleasepool {
        NSNumber *disconnected = @(NEVPNStatusDisconnected);
        NSNumber *connecting = @(NEVPNStatusConnecting);
        NSNumber *connected = @(NEVPNStatusConnected);
        NSNumber *disconnecting = @(NEVPNStatusDisconnecting);
        NSNumber *reasserting = @(NEVPNStatusReasserting);
        NSNumber *invalid = @(NEVPNStatusInvalid);
        for (int iteration = 0; iteration < 10; iteration++) {
            WaitOutcome start = run(YES, @[disconnected, disconnected, connecting, connecting, connected], 20);
            assert(start.result == VoyaTunnelReady && start.elapsed == 1.0);
            WaitOutcome stop = run(NO, @[connected, disconnecting, disconnected], 10);
            assert(stop.result == VoyaTunnelDisconnected && stop.elapsed == 1.5);
        }
        assert(run(YES, @[disconnected, connecting, disconnected], 20).result == VoyaTunnelDisconnected);
        assert(run(YES, @[disconnected, reasserting, disconnected], 20).result == VoyaTunnelDisconnected);
        WaitOutcome rejected = run(YES, @[invalid], 20);
        assert(rejected.result == VoyaTunnelInvalid && rejected.elapsed == 0);
        for (NSNumber *state in @[disconnected, connecting]) {
            WaitOutcome timeout = run(YES, @[state], 20);
            assert(timeout.result == VoyaTunnelTimedOut && timeout.elapsed == 20);
        }
        WaitOutcome delayed = run(NO, @[disconnected, disconnected, connecting, connected, disconnecting, disconnected], 10);
        assert(delayed.result == VoyaTunnelDisconnected && delayed.elapsed == 2.25);
        WaitOutcome stuck = run(NO, @[connected], 10);
        assert(stuck.result == VoyaTunnelTimedOut && stuck.elapsed == 10);
        assert(run(NO, @[disconnected, disconnected, connected], 10).result == VoyaTunnelTimedOut);
        // A queued start more than a second after the first disconnected sample
        // must not escape ownership after premature cleanup success.
        NSMutableArray *lateStates = [NSMutableArray array];
        for (int sample = 0; sample < 38; sample++) [lateStates addObject:disconnected];
        [lateStates addObject:connected];
        WaitOutcome late = run(NO, lateStates, 10);
        assert(late.result == VoyaTunnelTimedOut && late.elapsed == 10);
        WaitOutcome quiet = run(NO, @[disconnected], 10);
        assert(quiet.result == VoyaTunnelDisconnected && quiet.elapsed == 10);
        // A transient disconnected sample cannot satisfy the quiet interval.
        WaitOutcome bounced = run(NO, @[connected, disconnected, connecting, disconnected], 10);
        assert(bounced.result == VoyaTunnelDisconnected && bounced.elapsed == 1.75);
        puts("PacketTunnel waiter: startup races, failures, cleanup and 10 reconnect cycles passed.");
    }
    return 0;
}
