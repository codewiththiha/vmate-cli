use super::*;

#[test]
fn budget_drops_after_max_failures() {
    let mut b = RetryBudget::new(2);
    assert_eq!(b.on_failure(), RetryDecision::Retry);
    assert_eq!(b.on_failure(), RetryDecision::Drop);
}

#[test]
fn budget_retry_count_is_configurable() {
    let mut b = RetryBudget::new(3);
    assert_eq!(b.on_failure(), RetryDecision::Retry);
    assert_eq!(b.on_failure(), RetryDecision::Retry);
    assert_eq!(b.on_failure(), RetryDecision::Drop);
}

#[test]
fn budget_reset_lets_a_stable_session_retry_again() {
    // A long-lived session's crash resets the budget, so two network
    // blips never delete a working config.
    let mut b = RetryBudget::new(2);
    assert_eq!(b.on_failure(), RetryDecision::Retry);
    b.reset();
    assert_eq!(b.on_failure(), RetryDecision::Retry);
    assert_eq!(b.on_failure(), RetryDecision::Drop);
}
