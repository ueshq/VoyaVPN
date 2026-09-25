use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;
use voya_contracts::{InvalidationScope, QueryInvalidation};

use super::*;

/// One channel as `packages/contracts/events.json` declares it.
#[derive(Debug, Deserialize)]
struct ChannelShape {
    channel: String,
}

/// Generated from the same `bindings.ts` the frontend's types come from, so a
/// payload variant renamed in the shell fails here rather than on a device.
const EVENT_SHAPES: &str = include_str!("../../../../packages/contracts/events.json");

fn declared_shapes() -> BTreeMap<String, ChannelShape> {
    serde_json::from_str(EVENT_SHAPES).expect("events.json is valid JSON")
}

#[test]
fn every_channel_carries_the_wire_name_the_contract_declares() {
    let shapes = declared_shapes();

    for (key, channel) in [
        ("appEvent", EventChannel::App),
        ("invalidateEvent", EventChannel::Invalidate),
        ("transientStreamEvent", EventChannel::TransientStream),
    ] {
        let declared = shapes.get(key).expect("channel is declared in events.json");
        assert_eq!(declared.channel, channel.wire_name(), "channel {key}");
    }

    assert_eq!(shapes.len(), 3, "a channel was added to the contract");
}

#[test]
fn an_invalidation_serializes_as_the_frontend_reads_it() {
    let event = InvalidateEvent {
        keys: vec![QueryInvalidation {
            scope: InvalidationScope::Profiles,
            reason: "set_active_profile".to_string(),
        }],
    };

    assert_eq!(
        serde_json::to_value(&event).expect("serializes"),
        json!({ "keys": [{ "scope": { "kind": "profiles" }, "reason": "set_active_profile" }] })
    );
}
