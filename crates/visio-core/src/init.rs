//! Phased initialization for Visio Mobile.
//!
//! Four explicit phases (Settings, Auth, Services, Ready) with a progress
//! listener callback. Each phase can fail independently, allowing the native
//! UI to show granular startup progress.

use crate::events::{InitPhase, InitPhaseError, InitResult};
use crate::settings::SettingsStore;
use std::sync::Arc;

/// Callback interface for receiving initialization progress events.
pub trait InitProgressListener: Send + Sync {
    fn on_phase_started(&self, phase: InitPhase);
    fn on_phase_completed(
        &self,
        phase: InitPhase,
        result: InitResult,
        error: Option<InitPhaseError>,
    );
}

/// A no-op listener used by the legacy `VisioClient::new()` constructor.
pub struct NoOpListener;

impl InitProgressListener for NoOpListener {
    fn on_phase_started(&self, _phase: InitPhase) {}
    fn on_phase_completed(
        &self,
        _phase: InitPhase,
        _result: InitResult,
        _error: Option<InitPhaseError>,
    ) {
    }
}

/// Drives the four-phase initialization sequence, reporting progress
/// to a listener at each step.
pub struct InitSequence {
    listener: Box<dyn InitProgressListener>,
}

impl InitSequence {
    pub fn new(listener: Box<dyn InitProgressListener>) -> Self {
        Self { listener }
    }

    /// Phase 1: Load or create the settings store.
    pub fn init_settings(&self, data_dir: &str) -> Result<Arc<SettingsStore>, InitPhaseError> {
        self.listener.on_phase_started(InitPhase::Settings);
        let store = Arc::new(SettingsStore::new(data_dir));
        self.listener
            .on_phase_completed(InitPhase::Settings, InitResult::Success, None);
        Ok(store)
    }

    /// Phase 2: Restore saved authentication sessions.
    ///
    /// Currently a placeholder — reports success immediately. Will be fleshed
    /// out once persistent session storage is implemented.
    pub async fn init_auth(&self) -> InitResult {
        self.listener.on_phase_started(InitPhase::Auth);
        let result = InitResult::Success;
        self.listener
            .on_phase_completed(InitPhase::Auth, result.clone(), None);
        result
    }

    /// Phase 3: Start background services (calendar refresh, etc.).
    pub async fn init_services(&self) -> InitResult {
        self.listener.on_phase_started(InitPhase::Services);
        let result = InitResult::Success;
        self.listener
            .on_phase_completed(InitPhase::Services, result.clone(), None);
        result
    }

    /// Phase 4: Signal that initialization is complete.
    pub fn ready(&self) {
        self.listener.on_phase_started(InitPhase::Ready);
        self.listener
            .on_phase_completed(InitPhase::Ready, InitResult::Success, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RecordingListener {
        events: Mutex<Vec<(InitPhase, bool)>>,
    }

    impl RecordingListener {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                events: Mutex::new(Vec::new()),
            })
        }
    }

    impl InitProgressListener for RecordingListener {
        fn on_phase_started(&self, phase: InitPhase) {
            self.events.lock().unwrap().push((phase, true));
        }
        fn on_phase_completed(
            &self,
            phase: InitPhase,
            _result: InitResult,
            _error: Option<InitPhaseError>,
        ) {
            self.events.lock().unwrap().push((phase, false));
        }
    }

    // Arc<RecordingListener> implements InitProgressListener via the blanket
    // impl below, so it can be boxed and passed to InitSequence while
    // retaining a clone for assertions.
    impl InitProgressListener for Arc<RecordingListener> {
        fn on_phase_started(&self, phase: InitPhase) {
            (**self).on_phase_started(phase);
        }
        fn on_phase_completed(
            &self,
            phase: InitPhase,
            result: InitResult,
            error: Option<InitPhaseError>,
        ) {
            (**self).on_phase_completed(phase, result, error);
        }
    }

    #[test]
    fn test_settings_phase_reports_start_and_complete() {
        let listener = RecordingListener::new();
        let seq = InitSequence::new(Box::new(Arc::clone(&listener)));
        let dir = tempfile::tempdir().unwrap();
        let _ = seq.init_settings(dir.path().to_str().unwrap()).unwrap();
        let events = listener.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], (InitPhase::Settings, true));
        assert_eq!(events[1], (InitPhase::Settings, false));
    }

    #[test]
    fn test_full_init_sequence_phases_in_order() {
        let listener = RecordingListener::new();
        let seq = InitSequence::new(Box::new(Arc::clone(&listener)));
        let dir = tempfile::tempdir().unwrap();
        let _ = seq.init_settings(dir.path().to_str().unwrap()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            seq.init_auth().await;
            seq.init_services().await;
        });
        seq.ready();
        let events = listener.events.lock().unwrap();
        let phases: Vec<&InitPhase> = events.iter().map(|(p, _)| p).collect();
        assert_eq!(
            phases,
            vec![
                &InitPhase::Settings,
                &InitPhase::Settings,
                &InitPhase::Auth,
                &InitPhase::Auth,
                &InitPhase::Services,
                &InitPhase::Services,
                &InitPhase::Ready,
                &InitPhase::Ready,
            ]
        );
    }

    #[test]
    fn test_noop_listener_does_not_panic() {
        let seq = InitSequence::new(Box::new(NoOpListener));
        let dir = tempfile::tempdir().unwrap();
        let _ = seq.init_settings(dir.path().to_str().unwrap()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            seq.init_auth().await;
            seq.init_services().await;
        });
        seq.ready();
    }
}
