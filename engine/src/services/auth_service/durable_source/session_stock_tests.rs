use super::{DurableAuthError, SessionStockLimits};

#[test]
fn session_stock_limits_preserve_explicit_values_at_supported_boundaries() {
    for (per_account, total) in [(1, 1), (1, 4096), (64, 64), (64, 4096)] {
        let limits = SessionStockLimits::new(per_account, total)
            .unwrap_or_else(|_| panic!("valid trusted Session limits rejected"));
        let copied = limits;
        assert_eq!(limits.per_account(), per_account);
        assert_eq!(copied.total(), total);
    }
}

#[test]
fn session_stock_limits_reject_zero_or_excessive_account_stock() {
    for per_account in [0, 65, u16::MAX] {
        assert!(matches!(
            SessionStockLimits::new(per_account, 4096),
            Err(DurableAuthError::InvalidInput)
        ));
    }
}

#[test]
fn session_stock_limits_reject_invalid_total_or_account_greater_than_total() {
    for (per_account, total) in [(1, 0), (1, 4097), (1, u16::MAX), (2, 1), (64, 63)] {
        assert!(matches!(
            SessionStockLimits::new(per_account, total),
            Err(DurableAuthError::InvalidInput)
        ));
    }
}
