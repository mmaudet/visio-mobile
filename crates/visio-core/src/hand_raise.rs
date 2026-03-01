use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::Room;

use crate::errors::VisioError;
use crate::events::{EventEmitter, VisioEvent};

/// Manages hand-raise state using LiveKit participant attributes.
///
/// Interoperable with LaSuite Meet: uses `{"handRaisedAt": "<ISO 8601>"}` attribute.
/// Maintains a queue ordered by raise time (BTreeMap<timestamp, participant_sid>).
/// Supports auto-lower: if the local participant speaks for 3 consecutive seconds
/// with hand raised, the hand is automatically lowered.
pub struct HandRaiseManager {
    room: Arc<Room>,
    emitter: EventEmitter,
    /// timestamp -> participant_sid, ordered by raise time
    raised_hands: Arc<Mutex<BTreeMap<i64, String>>>,
    auto_lower_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl HandRaiseManager {
    pub fn new(room: Arc<Room>, emitter: EventEmitter) -> Self {
        Self {
            room,
            emitter,
            raised_hands: Arc::new(Mutex::new(BTreeMap::new())),
            auto_lower_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Raise the local participant's hand.
    ///
    /// Sets the `handRaisedAt` participant attribute to the current ISO 8601 timestamp,
    /// matching the LaSuite Meet protocol for interoperability.
    pub async fn raise_hand(&self) -> Result<(), VisioError> {
        let now = chrono::Utc::now();
        let iso_timestamp = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let epoch_ms = now.timestamp_millis();
        tracing::info!("raise_hand: setting handRaisedAt={iso_timestamp}");
        self.room
            .local_participant()
            .set_attributes(HashMap::from([("handRaisedAt".to_string(), iso_timestamp)]))
            .await
            .map_err(|e| {
                tracing::error!("raise_hand: set_attributes failed: {e}");
                VisioError::Room(e.to_string())
            })?;
        tracing::info!("raise_hand: set_attributes succeeded");

        let ts: i64 = epoch_ms;
        let local_sid = self.room.local_participant().sid().to_string();
        let mut hands = self.raised_hands.lock().await;
        hands.insert(ts, local_sid.clone());
        let position = hands.values().position(|s| s == &local_sid).unwrap_or(0) as u32 + 1;
        drop(hands);

        self.emitter.emit(VisioEvent::HandRaisedChanged {
            participant_sid: local_sid,
            raised: true,
            position,
        });
        Ok(())
    }

    /// Lower the local participant's hand.
    ///
    /// Clears the `handRaisedAt` attribute and cancels any auto-lower timer.
    pub async fn lower_hand(&self) -> Result<(), VisioError> {
        tracing::info!("lower_hand: clearing handRaisedAt attribute");
        self.room
            .local_participant()
            .set_attributes(HashMap::from([("handRaisedAt".to_string(), String::new())]))
            .await
            .map_err(|e| {
                tracing::error!("lower_hand: set_attributes failed: {e}");
                VisioError::Room(e.to_string())
            })?;
        tracing::info!("lower_hand: set_attributes succeeded");

        let local_sid = self.room.local_participant().sid().to_string();
        let mut hands = self.raised_hands.lock().await;
        hands.retain(|_, sid| sid != &local_sid);
        drop(hands);

        self.emitter.emit(VisioEvent::HandRaisedChanged {
            participant_sid: local_sid,
            raised: false,
            position: 0,
        });

        // Cancel auto-lower timer if running
        if let Some(handle) = self.auto_lower_handle.lock().await.take() {
            handle.abort();
        }
        Ok(())
    }

    /// Check if the local participant's hand is currently raised.
    pub async fn is_hand_raised(&self) -> bool {
        let local_sid = self.room.local_participant().sid().to_string();
        let hands = self.raised_hands.lock().await;
        hands.values().any(|sid| sid == &local_sid)
    }

    /// Handle a remote (or local) participant's attribute change.
    ///
    /// Called from the room event loop when `ParticipantAttributesChanged` fires.
    /// Checks the `handRaisedAt` key: non-empty means raised, empty means lowered.
    /// Parses ISO 8601 timestamps for ordering; falls back to epoch millis for compat.
    pub async fn handle_participant_attributes(
        &self,
        participant_sid: String,
        attributes: &HashMap<String, String>,
    ) {
        tracing::info!(
            "handle_participant_attributes: sid={participant_sid} attributes={attributes:?}"
        );
        let hand_raised_value = attributes.get("handRaisedAt").cloned().unwrap_or_default();
        let is_raised = !hand_raised_value.is_empty();
        tracing::info!(
            "handle_participant_attributes: sid={participant_sid} handRaisedAt={hand_raised_value:?} is_raised={is_raised}"
        );

        let mut hands = self.raised_hands.lock().await;
        if is_raised {
            // Parse ISO 8601 → epoch ms; fallback to raw integer parse for compat
            let ts: i64 = chrono::DateTime::parse_from_rfc3339(&hand_raised_value)
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_else(|_| hand_raised_value.parse().unwrap_or(0));
            if !hands.values().any(|s| s == &participant_sid) {
                hands.insert(ts, participant_sid.clone());
            }
        } else {
            hands.retain(|_, sid| sid != &participant_sid);
        }
        let position = if is_raised {
            hands.values().position(|s| s == &participant_sid).unwrap_or(0) as u32 + 1
        } else {
            0
        };
        drop(hands);

        tracing::info!(
            "handle_participant_attributes: emitting HandRaisedChanged sid={participant_sid} raised={is_raised} position={position}"
        );
        self.emitter.emit(VisioEvent::HandRaisedChanged {
            participant_sid,
            raised: is_raised,
            position,
        });
    }

    /// Update auto-lower state based on active speakers.
    ///
    /// If the local participant is speaking AND has their hand raised,
    /// starts a 3-second timer. If they are still speaking when the timer
    /// fires, the hand is automatically lowered.
    /// If the local participant stops speaking or their hand is not raised,
    /// any existing timer is cancelled.
    pub fn start_auto_lower(&self, active_speakers: Vec<String>) {
        let local_sid = self.room.local_participant().sid().to_string();
        let is_speaking = active_speakers.contains(&local_sid);

        let raised_hands = self.raised_hands.clone();
        let auto_lower_handle = self.auto_lower_handle.clone();
        let room = self.room.clone();
        let emitter = self.emitter.clone();

        tokio::spawn(async move {
            // Cancel existing timer
            if let Some(handle) = auto_lower_handle.lock().await.take() {
                handle.abort();
            }

            if !is_speaking {
                return;
            }

            // Check if hand is raised
            let hand_raised = {
                let hands = raised_hands.lock().await;
                hands.values().any(|sid| sid == &local_sid)
            };

            if !hand_raised {
                return;
            }

            // Start 3-second timer
            let local_sid2 = local_sid.clone();
            let raised_hands2 = raised_hands.clone();
            let room2 = room.clone();
            let emitter2 = emitter.clone();

            let handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                // Re-check hand is still raised after timer
                let still_raised = {
                    let hands = raised_hands2.lock().await;
                    hands.values().any(|sid| sid == &local_sid2)
                };

                if still_raised {
                    // Auto-lower: set attribute and update local state
                    let _ = room2
                        .local_participant()
                        .set_attributes(HashMap::from([
                            ("handRaisedAt".to_string(), String::new()),
                        ]))
                        .await;

                    let mut hands = raised_hands2.lock().await;
                    hands.retain(|_, sid| sid != &local_sid2);
                    drop(hands);

                    emitter2.emit(VisioEvent::HandRaisedChanged {
                        participant_sid: local_sid2,
                        raised: false,
                        position: 0,
                    });
                }
            });

            *auto_lower_handle.lock().await = Some(handle);
        });
    }

    /// Clear all hand-raise state (on disconnect).
    pub async fn clear(&self) {
        self.raised_hands.lock().await.clear();
        if let Some(handle) = self.auto_lower_handle.lock().await.take() {
            handle.abort();
        }
    }
}
