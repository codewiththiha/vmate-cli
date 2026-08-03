//! The connect candidate queue with defer-and-shuffle skip behaviour.

use crate::db::models::StoredConfig;
use rand::seq::SliceRandom;
use std::collections::VecDeque;
use std::path::Path;

/// A single candidate for the connect loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: i64,
    pub path: String,
    pub country: String,
}

impl Candidate {
    /// The config's file name for display, falling back to the country when the
    /// path has no file-name component.
    pub fn file_name(&self) -> String {
        Path::new(&self.path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.country.clone())
    }
}

impl From<&StoredConfig> for Candidate {
    fn from(config: &StoredConfig) -> Self {
        Candidate {
            id: config.id,
            path: config.path.clone(),
            country: config.country.clone(),
        }
    }
}

/// A FIFO of pending candidates plus a deferred backlog.
///
/// Skipped candidates are *not* removed — they are pushed onto the deferred
/// queue and, once the pending queue is exhausted, all deferred candidates are
/// shuffled and promoted back. This delays re-trying a skipped config without
/// dropping it from history.
#[derive(Debug, Default)]
pub struct ConnectQueue {
    pending: VecDeque<Candidate>,
    deferred: VecDeque<Candidate>,
}

impl ConnectQueue {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self {
            pending: candidates.into(),
            deferred: VecDeque::new(),
        }
    }

    pub fn from_stored(configs: Vec<StoredConfig>) -> Self {
        Self::new(configs.iter().map(Candidate::from).collect())
    }

    pub fn len(&self) -> usize {
        self.pending.len() + self.deferred.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.deferred.is_empty()
    }

    /// Pop the next candidate, refilling from the shuffled deferred backlog
    /// when the pending queue is exhausted.
    pub fn next_candidate(&mut self) -> Option<Candidate> {
        if self.pending.is_empty() {
            self.refill_deferred();
        }
        self.pending.pop_front()
    }

    /// Defer a candidate to the end of the session queue.
    pub fn skip(&mut self, candidate: Candidate) {
        self.deferred.push_back(candidate);
    }

    fn refill_deferred(&mut self) {
        let mut deferred: Vec<Candidate> = self.deferred.drain(..).collect();
        deferred.shuffle(&mut rand::thread_rng());
        self.pending.extend(deferred);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(n: i64) -> Vec<Candidate> {
        (1..=n)
            .map(|i| Candidate {
                id: i,
                path: format!("/tmp/c{i}.ovpn"),
                country: "JP".to_string(),
            })
            .collect()
    }

    #[test]
    fn pops_in_order() {
        let mut queue = ConnectQueue::new(candidates(3));
        assert_eq!(queue.next_candidate().unwrap().id, 1);
        assert_eq!(queue.next_candidate().unwrap().id, 2);
        assert_eq!(queue.next_candidate().unwrap().id, 3);
        assert!(queue.next_candidate().is_none());
    }

    #[test]
    fn skipping_defers_but_keeps() {
        let mut queue = ConnectQueue::new(candidates(3));
        let first = queue.next_candidate().unwrap(); // id 1
        assert_eq!(queue.len(), 2);
        queue.skip(first);
        // Skipping defers, never removes: 2 pending + 1 deferred.
        assert_eq!(queue.len(), 3);
        let second = queue.next_candidate().unwrap(); // id 2
        let third = queue.next_candidate().unwrap(); // id 3
        assert_eq!((second.id, third.id), (2, 3));
        // Pending exhausted -> refilled from deferred (shuffled).
        let back = queue.next_candidate().unwrap();
        assert_eq!(back.id, 1);
        assert!(queue.next_candidate().is_none());
    }

    #[test]
    fn all_candidates_survive_skips() {
        let mut queue = ConnectQueue::new(candidates(5));
        let mut seen = std::collections::HashSet::new();
        let mut rounds = 0;
        while let Some(c) = queue.next_candidate() {
            rounds += 1;
            assert!(rounds <= 30, "queue never drains");
            // Skip even-numbered candidates; they must all come back.
            if seen.insert(c.id) && rounds % 2 == 0 {
                queue.skip(c);
            }
        }
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn from_stored_maps_ids() {
        let stored: Vec<StoredConfig> = (1..=2)
            .map(|i| StoredConfig {
                id: i,
                path: format!("/tmp/s{i}.ovpn"),
                path_sha256: String::new(),
                remote_host: None,
                country: "KR".to_string(),
                country_source: String::new(),
                status: crate::db::models::ConfigStatus::Success,
                success_count: 1,
                failure_count: 0,
                skipped_count: 0,
                last_success_at: None,
                last_failure_at: None,
                last_skipped_at: None,
                last_error: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .collect();
        let mut queue = ConnectQueue::from_stored(stored);
        let first = queue.next_candidate().unwrap();
        assert_eq!(first.id, 1);
        assert_eq!(first.country, "KR");
    }
}
