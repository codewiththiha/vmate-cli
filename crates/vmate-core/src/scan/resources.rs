//! Bound probe concurrency by the process file-descriptor budget.

/// Return a conservative concurrency ceiling while reserving runtime headroom.
pub(super) fn workers(requested: usize) -> usize {
    let soft = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
        .map(|(soft, _)| soft as usize)
        .unwrap_or(1024);
    let allowed = requested.max(1).min(descriptor_budget(soft));
    if allowed < requested {
        tracing::warn!(
            requested,
            allowed,
            "scan concurrency reduced for file-descriptor headroom"
        );
    }
    allowed
}

fn descriptor_budget(soft: usize) -> usize {
    soft.saturating_sub(128).saturating_div(6).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_headroom_even_with_small_limits() {
        assert_eq!(descriptor_budget(1024), 149);
        assert_eq!(descriptor_budget(64), 1);
        assert!(descriptor_budget(8192) >= 500);
    }
}
