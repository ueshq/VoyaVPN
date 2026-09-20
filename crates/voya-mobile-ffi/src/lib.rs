//! The mobile host.
//!
//! This crate is to `apps/mobile` what `apps/desktop/src-tauri` is to
//! `apps/desktop/src`: it owns a tokio runtime, opens the database through
//! `AppServices::connect`, injects the dependencies the managers need, and
//! turns `voya-app`'s event sinks into the three channels the frontend
//! subscribes to. `voya-app` itself learns nothing about phones beyond the
//! `TargetOs` variants ADR 0012 added.
//!
//! The uniffi surface is an envelope, not a model: one `invoke(command,
//! args_json)` in, JSON out, in exactly the wire shape Tauri uses. See ADR 0012
//! for why, and for the two tests that stand in for the types uniffi is not
//! checking.

uniffi::setup_scaffolding!();

pub mod app;
pub mod dispatch;
pub mod events;
pub mod handoff;
pub mod sinks;
pub mod tunnel;

pub use app::{CommandError, StartupError, VoyaApp};
pub use events::{AppEvent, EventChannel, InvalidateEvent, TransientStreamEvent};
pub use sinks::EventListener;
pub use tunnel::{HostTunController, TunnelError, TunnelHost};
