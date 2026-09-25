use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
use voya_contracts::{AppEvent, InvalidateEvent, TransientStreamEvent};

static NEXT_LOG_LINE_ID: AtomicU32 = AtomicU32::new(1);

pub fn next_log_line_id() -> u32 {
    NEXT_LOG_LINE_ID.fetch_add(1, Ordering::Relaxed)
}

// `Event` is tauri-specta's trait and the payloads are voya-contracts' types,
// so the orphan rule rules out deriving one on the other. Each channel is a
// newtype that is transparent on the wire and in the bindings, and keeps the
// channel name the frontend subscribes to.

#[derive(Debug, Clone, Deserialize, Serialize, Type, Event)]
#[serde(transparent)]
#[specta(transparent)]
#[tauri_specta(event_name = "invalidate-event")]
pub struct InvalidateChannel(pub InvalidateEvent);

#[derive(Debug, Clone, Deserialize, Serialize, Type, Event)]
#[serde(transparent)]
#[specta(transparent)]
#[tauri_specta(event_name = "transient-stream-event")]
pub struct TransientStreamChannel(pub TransientStreamEvent);

#[derive(Debug, Clone, Deserialize, Serialize, Type, Event)]
#[serde(transparent)]
#[specta(transparent)]
#[tauri_specta(event_name = "app-event")]
pub struct AppChannel(pub AppEvent);

/// Puts a contract payload on its channel.
pub trait Emit {
    fn emit<R: tauri::Runtime>(&self, app: &tauri::AppHandle<R>) -> tauri::Result<()>;
}

impl Emit for InvalidateEvent {
    fn emit<R: tauri::Runtime>(&self, app: &tauri::AppHandle<R>) -> tauri::Result<()> {
        Event::emit(&InvalidateChannel(self.clone()), app)
    }
}

impl Emit for TransientStreamEvent {
    fn emit<R: tauri::Runtime>(&self, app: &tauri::AppHandle<R>) -> tauri::Result<()> {
        Event::emit(&TransientStreamChannel(self.clone()), app)
    }
}

impl Emit for AppEvent {
    fn emit<R: tauri::Runtime>(&self, app: &tauri::AppHandle<R>) -> tauri::Result<()> {
        Event::emit(&AppChannel(self.clone()), app)
    }
}
