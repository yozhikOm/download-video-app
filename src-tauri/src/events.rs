//! Backend → UI events. The backend owns the state; the UI only shows what
//! arrives here and sends commands back.

use serde::Serialize;

use crate::download::Download;
use crate::probe::ProbeState;
use crate::settings::SettingsState;
use crate::toolbox::SetupState;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum AppEvent {
    Setup(SetupState),
    Probe(ProbeState),
    Settings(SettingsState),
    /// The Downloads in the Queue, in the order they run.
    Queue(Vec<Download>),
}

pub trait EventSink: Send + Sync {
    fn send(&self, event: AppEvent);
}

#[cfg(test)]
#[derive(Default)]
pub struct RecordingSink(std::sync::Mutex<Vec<AppEvent>>);

#[cfg(test)]
impl RecordingSink {
    pub fn events(&self) -> Vec<AppEvent> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl EventSink for RecordingSink {
    fn send(&self, event: AppEvent) {
        self.0.lock().unwrap().push(event);
    }
}
