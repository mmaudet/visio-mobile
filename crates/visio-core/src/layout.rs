//! Layout engine: participant sorting for the video grid.
//!
//! Sort order: hand raised > voice activity (10s anti-flicker) > camera on > join time.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::events::ParticipantInfo;

/// Two participants who both spoke within this window are sorted by identity
/// (alphabetical) instead of by recency, preventing rapid tile swaps.
const ANTI_FLICKER_WINDOW: Duration = Duration::from_secs(10);

/// Sort participants in-place for the video grid layout.
///
/// Priority (highest first):
/// 1. Hand raised (raised before not-raised)
/// 2. Voice activity — recent speaker first, with anti-flicker: if both
///    participants spoke within the last 10 s, sort by identity for stability.
/// 3. Camera on before camera off
/// 4. Join time ascending (earlier joiners first)
pub fn sort_participants(participants: &mut [ParticipantInfo]) {
    let now = Instant::now();
    participants.sort_by(|a, b| {
        // 1. Hand raised first
        let hand_cmp = b.hand_raised.cmp(&a.hand_raised);
        if hand_cmp != std::cmp::Ordering::Equal {
            return hand_cmp;
        }

        // 2. Voice activity with anti-flicker
        let a_recent = a
            .last_spoke_at
            .map_or(false, |t| now.duration_since(t) < ANTI_FLICKER_WINDOW);
        let b_recent = b
            .last_spoke_at
            .map_or(false, |t| now.duration_since(t) < ANTI_FLICKER_WINDOW);

        match (a_recent, b_recent) {
            (true, true) => {
                // Both within anti-flicker window — sort by identity for stability
                let id_cmp = a.identity.cmp(&b.identity);
                if id_cmp != std::cmp::Ordering::Equal {
                    return id_cmp;
                }
            }
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            (false, false) => {
                // Neither within window — sort by recency (more recent first)
                match (a.last_spoke_at, b.last_spoke_at) {
                    (Some(at), Some(bt)) => {
                        // More recent = larger Instant = should come first
                        let spoke_cmp = bt.cmp(&at).reverse();
                        if spoke_cmp != std::cmp::Ordering::Equal {
                            return spoke_cmp;
                        }
                    }
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (None, None) => {}
                }
            }
        }

        // 3. Camera on before camera off
        let cam_cmp = b.has_video.cmp(&a.has_video);
        if cam_cmp != std::cmp::Ordering::Equal {
            return cam_cmp;
        }

        // 4. Join time ascending (earlier joiners first)
        match (a.joined_at, b.joined_at) {
            (Some(at), Some(bt)) => at.cmp(&bt),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}

/// Layout mode for the video grid.
pub enum LayoutMode {
    /// Equal-sized tiles in a grid.
    Grid,
    /// One main participant + thumbnail strip.
    Speaker,
}

/// Thread-safe layout engine managing pagination and speaker mode.
pub struct LayoutEngine {
    inner: Mutex<LayoutEngineInner>,
}

struct LayoutEngineInner {
    sorted_order: Vec<String>,
    layout_mode: LayoutMode,
    current_page: usize,
    page_size: usize,
}

impl LayoutEngine {
    /// Create a new layout engine with default settings (page_size=4, page=0, Grid mode).
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(LayoutEngineInner {
                sorted_order: Vec::new(),
                layout_mode: LayoutMode::Grid,
                current_page: 0,
                page_size: 4,
            }),
        }
    }

    /// Update the sorted participant order. Clamps current page if needed.
    pub fn update_sorted_order(&self, order: Vec<String>) {
        let mut inner = self.inner.lock().unwrap();
        inner.sorted_order = order;
        clamp_page(&mut inner);
    }

    /// Set page size (minimum 1). Clamps current page if needed.
    pub fn set_page_size(&self, size: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.page_size = size.max(1);
        clamp_page(&mut inner);
    }

    /// Set current page (clamped to max valid page).
    pub fn set_current_page(&self, page: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.current_page = page;
        clamp_page(&mut inner);
    }

    /// Return the current page index.
    pub fn current_page(&self) -> usize {
        self.inner.lock().unwrap().current_page
    }

    /// Return total number of pages (0 if empty).
    pub fn page_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        page_count_inner(&inner)
    }

    /// Return participant SIDs visible on the current page.
    pub fn visible_participants(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let start = inner.current_page * inner.page_size;
        let end = (start + inner.page_size).min(inner.sorted_order.len());
        if start >= inner.sorted_order.len() {
            return Vec::new();
        }
        inner.sorted_order[start..end].to_vec()
    }

    /// Return participant SIDs on adjacent pages (±1) for pre-caching.
    /// Excludes participants on the current (visible) page.
    pub fn precached_participants(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let total = page_count_inner(&inner);
        if total == 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        for &adj_page in &[
            inner.current_page.wrapping_sub(1),
            inner.current_page + 1,
        ] {
            if adj_page < total && adj_page != inner.current_page {
                let start = adj_page * inner.page_size;
                let end = (start + inner.page_size).min(inner.sorted_order.len());
                result.extend_from_slice(&inner.sorted_order[start..end]);
            }
        }
        result
    }

    /// Set the layout mode (Grid or Speaker).
    pub fn set_layout_mode(&self, mode: LayoutMode) {
        self.inner.lock().unwrap().layout_mode = mode;
    }

    /// Check if the current mode is Speaker.
    pub fn is_speaker_mode(&self) -> bool {
        matches!(self.inner.lock().unwrap().layout_mode, LayoutMode::Speaker)
    }

}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn page_count_inner(inner: &LayoutEngineInner) -> usize {
    let len = inner.sorted_order.len();
    if len == 0 {
        return 0;
    }
    (len + inner.page_size - 1) / inner.page_size
}

fn clamp_page(inner: &mut LayoutEngineInner) {
    let max_page = page_count_inner(inner);
    if max_page == 0 {
        inner.current_page = 0;
    } else if inner.current_page >= max_page {
        inner.current_page = max_page - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ConnectionQuality;

    fn make(sid: &str, identity: &str) -> ParticipantInfo {
        ParticipantInfo {
            sid: sid.to_string(),
            identity: identity.to_string(),
            name: Some(identity.to_string()),
            is_muted: false,
            has_video: false,
            video_track_sid: None,
            has_screen_share: false,
            screen_share_track_sid: None,
            connection_quality: ConnectionQuality::Good,
            color: None,
            is_admin: false,
            last_spoke_at: None,
            joined_at: Some(Instant::now()),
            hand_raised: false,
        }
    }

    #[test]
    fn test_hand_raised_first() {
        let mut a = make("s1", "alice");
        let mut b = make("s2", "bob");
        b.hand_raised = true;
        a.hand_raised = false;

        let mut list = vec![a, b];
        sort_participants(&mut list);

        assert_eq!(list[0].identity, "bob", "hand-raised should be first");
    }

    #[test]
    fn test_voice_activity_ordering() {
        let now = Instant::now();
        let mut recent = make("s1", "alice");
        recent.last_spoke_at = Some(now - Duration::from_secs(2));

        let mut older = make("s2", "bob");
        older.last_spoke_at = Some(now - Duration::from_secs(15));

        let never = make("s3", "charlie");

        let mut list = vec![never, older, recent];
        sort_participants(&mut list);

        assert_eq!(list[0].identity, "alice", "recent speaker first");
        assert_eq!(list[1].identity, "bob", "older speaker second");
        assert_eq!(list[2].identity, "charlie", "never-spoke last");
    }

    #[test]
    fn test_anti_flicker_stable_sort() {
        let now = Instant::now();
        let mut a = make("s1", "bob");
        a.last_spoke_at = Some(now - Duration::from_secs(3));

        let mut b = make("s2", "alice");
        b.last_spoke_at = Some(now - Duration::from_secs(5));

        // Both within 10s window — should sort by identity (alice < bob)
        let mut list = vec![a, b];
        sort_participants(&mut list);

        assert_eq!(list[0].identity, "alice", "anti-flicker: alphabetical by identity");
        assert_eq!(list[1].identity, "bob");
    }

    #[test]
    fn test_anti_flicker_expires() {
        let now = Instant::now();
        let mut recent = make("s1", "bob");
        recent.last_spoke_at = Some(now - Duration::from_secs(3));

        let mut old = make("s2", "alice");
        old.last_spoke_at = Some(now - Duration::from_secs(15));

        // bob is within 10s, alice is outside — bob should come first
        let mut list = vec![old, recent];
        sort_participants(&mut list);

        assert_eq!(list[0].identity, "bob", "recent speaker first when other expired");
        assert_eq!(list[1].identity, "alice");
    }

    #[test]
    fn test_camera_tiebreaker() {
        let joined = Instant::now();
        let mut cam_on = make("s1", "alice");
        cam_on.has_video = true;
        cam_on.joined_at = Some(joined);

        let mut cam_off = make("s2", "bob");
        cam_off.has_video = false;
        cam_off.joined_at = Some(joined);

        let mut list = vec![cam_off, cam_on];
        sort_participants(&mut list);

        assert_eq!(list[0].identity, "alice", "camera on should come first");
        assert_eq!(list[1].identity, "bob");
    }

    #[test]
    fn test_100_participants_performance() {
        let now = Instant::now();
        let mut list: Vec<ParticipantInfo> = (0..100)
            .map(|i| {
                let mut p = make(&format!("s{i}"), &format!("user{i:03}"));
                if i % 3 == 0 {
                    p.last_spoke_at = Some(now - Duration::from_secs(i));
                }
                if i % 5 == 0 {
                    p.has_video = true;
                }
                if i % 20 == 0 {
                    p.hand_raised = true;
                }
                p
            })
            .collect();

        let start = Instant::now();
        sort_participants(&mut list);
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(1),
            "sorting 100 participants took {elapsed:?}, expected <1ms"
        );

        // Verify hand-raised participants come first
        let first_non_raised = list.iter().position(|p| !p.hand_raised).unwrap_or(list.len());
        for p in &list[..first_non_raised] {
            assert!(p.hand_raised);
        }
        for p in &list[first_non_raised..] {
            assert!(!p.hand_raised);
        }
    }

    // --- Pagination tests ---

    #[test]
    fn test_pagination_basic() {
        let engine = LayoutEngine::new();
        engine.set_page_size(4);
        let sids: Vec<String> = (0..12).map(|i| format!("sid_{i}")).collect();
        engine.update_sorted_order(sids);
        assert_eq!(engine.page_count(), 3);
        assert_eq!(
            engine.visible_participants(),
            vec!["sid_0", "sid_1", "sid_2", "sid_3"]
        );
    }

    #[test]
    fn test_pagination_last_page_partial() {
        let engine = LayoutEngine::new();
        engine.set_page_size(4);
        let sids: Vec<String> = (0..10).map(|i| format!("sid_{i}")).collect();
        engine.update_sorted_order(sids);
        engine.set_current_page(2);
        assert_eq!(engine.visible_participants(), vec!["sid_8", "sid_9"]);
    }

    #[test]
    fn test_pagination_precache() {
        let engine = LayoutEngine::new();
        engine.set_page_size(4);
        let sids: Vec<String> = (0..20).map(|i| format!("sid_{i}")).collect();
        engine.update_sorted_order(sids);
        engine.set_current_page(2);
        let precached = engine.precached_participants();
        assert!(precached.contains(&"sid_4".to_string())); // page 1
        assert!(precached.contains(&"sid_12".to_string())); // page 3
        assert!(!precached.contains(&"sid_8".to_string())); // visible, not precache
    }

    #[test]
    fn test_pagination_100_participants() {
        let engine = LayoutEngine::new();
        engine.set_page_size(4);
        let sids: Vec<String> = (0..100).map(|i| format!("sid_{i}")).collect();
        engine.update_sorted_order(sids);
        assert_eq!(engine.page_count(), 25);
        engine.set_current_page(24);
        assert_eq!(
            engine.visible_participants(),
            vec!["sid_96", "sid_97", "sid_98", "sid_99"]
        );
    }

    #[test]
    fn test_page_clamp_on_resize() {
        let engine = LayoutEngine::new();
        engine.set_page_size(4);
        let sids: Vec<String> = (0..12).map(|i| format!("sid_{i}")).collect();
        engine.update_sorted_order(sids);
        engine.set_current_page(2);
        engine.set_page_size(6);
        assert_eq!(engine.page_count(), 2);
        assert!(engine.current_page() <= 1);
    }

}
