//! The three event channels. The payloads are `voya-contracts` types shared
//! with the desktop shell; what this host owns is the wire name each channel
//! travels under, checked against `packages/contracts/events.json`.

pub use voya_contracts::{AppEvent, InvalidateEvent, TransientStreamEvent};

/// The wire name of a channel, as the frontend subscribes to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventChannel {
    App,
    Invalidate,
    TransientStream,
}

impl EventChannel {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::App => "app-event",
            Self::Invalidate => "invalidate-event",
            Self::TransientStream => "transient-stream-event",
        }
    }
}

#[cfg(test)]
mod tests;
