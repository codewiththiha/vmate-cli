//! In-memory IP -> country memoization layered over the SQLite cache.

use crate::country::CountryCode;
use std::collections::HashMap;
use std::sync::Mutex;

/// A small per-session cache that avoids re-hitting SQLite (and the network)
/// for the same IP within one scan.
#[derive(Default)]
pub struct GeoMemo {
    inner: Mutex<HashMap<String, CountryCode>>,
}

impl GeoMemo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, ip: &str) -> Option<CountryCode> {
        self.inner.lock().ok()?.get(ip).cloned()
    }

    pub fn set(&self, ip: String, country: CountryCode) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.insert(ip, country);
        }
    }
}
