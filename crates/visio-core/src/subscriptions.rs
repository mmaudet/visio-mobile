//! Subscription management with anti-jitter delays.
//!
//! Provides a decision matrix for desired video quality based on track
//! visibility and bandwidth mode, plus a [`SubscriptionManager`] that
//! applies hysteresis to prevent rapid subscribe/unsubscribe churn.

use crate::bandwidth::BandwidthMode;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Desired video quality for a track subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoQuality {
    High,
    Low,
    Off,
}

/// Visibility state of a video track in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackVisibility {
    /// Track is visible on screen.
    Visible,
    /// Track is not visible but kept warm for fast switching.
    Precached,
    /// Track is fully off-screen.
    OffScreen,
}

/// Desired quality for a camera track given its visibility and bandwidth mode.
pub fn desired_quality(visibility: TrackVisibility, bandwidth: BandwidthMode) -> VideoQuality {
    match (visibility, bandwidth) {
        (_, BandwidthMode::AudioOnly) => VideoQuality::Off,
        (TrackVisibility::Visible, BandwidthMode::Full) => VideoQuality::High,
        (TrackVisibility::Visible, BandwidthMode::ReducedVideo) => VideoQuality::Low,
        (TrackVisibility::Precached, BandwidthMode::Full) => VideoQuality::Low,
        (TrackVisibility::Precached, BandwidthMode::ReducedVideo) => VideoQuality::Off,
        (TrackVisibility::OffScreen, _) => VideoQuality::Off,
    }
}

/// Screen share: always HIGH except in AudioOnly.
pub fn desired_screen_share_quality(bandwidth: BandwidthMode) -> VideoQuality {
    match bandwidth {
        BandwidthMode::AudioOnly => VideoQuality::Off,
        _ => VideoQuality::High,
    }
}

/// Subscription statistics snapshot.
#[derive(Debug, Clone)]
pub struct SubscriptionStats {
    pub subscribed_high: u32,
    pub subscribed_low: u32,
    pub unsubscribed: u32,
    pub screen_shares: u32,
}

/// Delay before an unsubscribe (any → Off) takes effect.
const UNSUBSCRIBE_DELAY: Duration = Duration::from_secs(2);

/// Delay before a quality downgrade (High → Low) takes effect.
const QUALITY_DOWNGRADE_DELAY: Duration = Duration::from_secs(5);

/// Manages track subscription states with anti-jitter delays.
///
/// Upgrades (Off→Low, Off→High, Low→High) are immediate.
/// Downgrades and unsubscribes are delayed to avoid churn from
/// transient layout changes or brief network dips.
pub struct SubscriptionManager {
    current: HashMap<String, VideoQuality>,
    pending: HashMap<String, (VideoQuality, Instant)>,
}

impl SubscriptionManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            current: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    /// Record the current applied state for a track, clearing any pending change.
    pub fn apply_current(&mut self, track_sid: &str, quality: VideoQuality) {
        self.current.insert(track_sid.to_owned(), quality);
        self.pending.remove(track_sid);
    }

    /// Request a quality change for a track.
    ///
    /// - Upgrades are applied immediately (returned via next `pending_actions` call with delay 0).
    /// - Downgrades and unsubscribes are delayed.
    /// - If the requested quality matches the current state, any pending change is cancelled.
    /// - A new request supersedes any existing pending change for the same track.
    pub fn request_change(&mut self, track_sid: &str, target: VideoQuality, now: Instant) {
        let current = self.current.get(track_sid).copied().unwrap_or(VideoQuality::Off);

        // Same as current: cancel any pending change.
        if target == current {
            self.pending.remove(track_sid);
            return;
        }

        let delay = change_delay(current, target);
        let fire_at = now + delay;
        self.pending.insert(track_sid.to_owned(), (target, fire_at));
    }

    /// Returns actions whose delay has elapsed, applying them to current state.
    pub fn pending_actions(&mut self, now: Instant) -> Vec<(String, VideoQuality)> {
        let ready: Vec<(String, VideoQuality)> = self
            .pending
            .iter()
            .filter(|(_, (_, fire_at))| now >= *fire_at)
            .map(|(sid, (quality, _))| (sid.clone(), *quality))
            .collect();

        for (sid, quality) in &ready {
            self.current.insert(sid.clone(), *quality);
            self.pending.remove(sid.as_str());
        }

        ready
    }

    /// Snapshot of current subscription counts.
    pub fn stats(&self) -> SubscriptionStats {
        let mut high = 0u32;
        let mut low = 0u32;
        let mut off = 0u32;

        for quality in self.current.values() {
            match quality {
                VideoQuality::High => high += 1,
                VideoQuality::Low => low += 1,
                VideoQuality::Off => off += 1,
            }
        }

        SubscriptionStats {
            subscribed_high: high,
            subscribed_low: low,
            unsubscribed: off,
            screen_shares: 0,
        }
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the delay for a quality transition.
fn change_delay(from: VideoQuality, to: VideoQuality) -> Duration {
    match (from, to) {
        // Upgrades: immediate.
        (VideoQuality::Off, VideoQuality::Low)
        | (VideoQuality::Off, VideoQuality::High)
        | (VideoQuality::Low, VideoQuality::High) => Duration::ZERO,
        // Unsubscribe: 2s delay.
        (_, VideoQuality::Off) => UNSUBSCRIBE_DELAY,
        // Quality downgrade (High → Low): 5s delay.
        (VideoQuality::High, VideoQuality::Low) => QUALITY_DOWNGRADE_DELAY,
        // Same quality — shouldn't reach here, but no delay.
        _ => Duration::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    // ── Decision matrix tests ──────────────────────────────────────────

    #[test]
    fn test_decision_matrix_full_mode() {
        assert_eq!(
            desired_quality(TrackVisibility::Visible, BandwidthMode::Full),
            VideoQuality::High
        );
        assert_eq!(
            desired_quality(TrackVisibility::Precached, BandwidthMode::Full),
            VideoQuality::Low
        );
        assert_eq!(
            desired_quality(TrackVisibility::OffScreen, BandwidthMode::Full),
            VideoQuality::Off
        );
    }

    #[test]
    fn test_decision_matrix_reduced_mode() {
        assert_eq!(
            desired_quality(TrackVisibility::Visible, BandwidthMode::ReducedVideo),
            VideoQuality::Low
        );
        assert_eq!(
            desired_quality(TrackVisibility::Precached, BandwidthMode::ReducedVideo),
            VideoQuality::Off
        );
        assert_eq!(
            desired_quality(TrackVisibility::OffScreen, BandwidthMode::ReducedVideo),
            VideoQuality::Off
        );
    }

    #[test]
    fn test_decision_matrix_audio_only() {
        assert_eq!(
            desired_quality(TrackVisibility::Visible, BandwidthMode::AudioOnly),
            VideoQuality::Off
        );
        assert_eq!(
            desired_quality(TrackVisibility::Precached, BandwidthMode::AudioOnly),
            VideoQuality::Off
        );
        assert_eq!(
            desired_quality(TrackVisibility::OffScreen, BandwidthMode::AudioOnly),
            VideoQuality::Off
        );
    }

    #[test]
    fn test_screen_share_always_high() {
        assert_eq!(
            desired_screen_share_quality(BandwidthMode::Full),
            VideoQuality::High
        );
        assert_eq!(
            desired_screen_share_quality(BandwidthMode::ReducedVideo),
            VideoQuality::High
        );
        assert_eq!(
            desired_screen_share_quality(BandwidthMode::AudioOnly),
            VideoQuality::Off
        );
    }

    // ── Anti-jitter tests ──────────────────────────────────────────────

    #[test]
    fn test_unsubscribe_delay() {
        let now = t0();
        let mut mgr = SubscriptionManager::new();

        // Start with a HIGH subscription.
        mgr.apply_current("t1", VideoQuality::High);

        // Request unsubscribe.
        mgr.request_change("t1", VideoQuality::Off, now);

        // At 1.5s: still pending.
        let actions = mgr.pending_actions(now + Duration::from_millis(1500));
        assert!(actions.is_empty());

        // At 2.1s: fires.
        let actions = mgr.pending_actions(now + Duration::from_millis(2100));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ("t1".to_owned(), VideoQuality::Off));
    }

    #[test]
    fn test_unsubscribe_cancelled() {
        let now = t0();
        let mut mgr = SubscriptionManager::new();

        mgr.apply_current("t1", VideoQuality::High);

        // Request unsubscribe.
        mgr.request_change("t1", VideoQuality::Off, now);

        // At 1s: resubscribe (same as current = High cancels pending).
        mgr.request_change("t1", VideoQuality::High, now + Duration::from_secs(1));

        // At 3s: nothing should fire.
        let actions = mgr.pending_actions(now + Duration::from_secs(3));
        assert!(actions.is_empty());
    }

    #[test]
    fn test_subscribe_immediate() {
        let now = t0();
        let mut mgr = SubscriptionManager::new();

        // Track starts Off (default).
        mgr.apply_current("t1", VideoQuality::Off);

        // Request subscribe to High.
        mgr.request_change("t1", VideoQuality::High, now);

        // Immediate: fires at t=0.
        let actions = mgr.pending_actions(now);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ("t1".to_owned(), VideoQuality::High));
    }

    #[test]
    fn test_quality_downgrade_delay() {
        let now = t0();
        let mut mgr = SubscriptionManager::new();

        mgr.apply_current("t1", VideoQuality::High);

        // Request downgrade to Low.
        mgr.request_change("t1", VideoQuality::Low, now);

        // At 4s: still pending.
        let actions = mgr.pending_actions(now + Duration::from_secs(4));
        assert!(actions.is_empty());

        // At 5.1s: fires.
        let actions = mgr.pending_actions(now + Duration::from_millis(5100));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ("t1".to_owned(), VideoQuality::Low));
    }

    #[test]
    fn test_stats() {
        let mut mgr = SubscriptionManager::new();

        mgr.apply_current("t1", VideoQuality::High);
        mgr.apply_current("t2", VideoQuality::High);
        mgr.apply_current("t3", VideoQuality::Low);
        mgr.apply_current("t4", VideoQuality::Off);

        let stats = mgr.stats();
        assert_eq!(stats.subscribed_high, 2);
        assert_eq!(stats.subscribed_low, 1);
        assert_eq!(stats.unsubscribed, 1);
        assert_eq!(stats.screen_shares, 0);
    }
}
