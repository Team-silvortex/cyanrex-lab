use super::*;

fn context() -> LoginContext {
    LoginContext::new(Uuid::from_u128(6201).try_into().unwrap())
}

#[test]
fn login_context_keeps_five_attempts_per_name_and_clears_only_confirmed_name() {
    let context = context();
    let user: LegacyUsername = "synthetic-user".parse().unwrap();
    for _ in 0..5 {
        assert_eq!(context.admit(&user), Ok(()));
    }
    assert_eq!(context.admit(&user), Err(DurableAuthError::RateLimited));
    let other: LegacyUsername = "synthetic-other".parse().unwrap();
    context.admit(&other).unwrap();
    context.clear(&user);
    context.admit(&user).unwrap();
    assert_eq!(
        context
            .attempts
            .lock()
            .unwrap()
            .get(other.as_str())
            .unwrap()
            .0,
        1
    );
}

#[test]
fn login_context_caps_distinct_names_and_expires_original_five_minute_window() {
    let context = context();
    for index in 0..1024 {
        context
            .admit(&format!("synthetic-{index}").parse().unwrap())
            .unwrap();
    }
    let overflow: LegacyUsername = "synthetic-overflow".parse().unwrap();
    assert_eq!(context.admit(&overflow), Err(DurableAuthError::RateLimited));
    context
        .attempts
        .lock()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()
        .1 = Instant::now()
        .checked_sub(Duration::from_secs(301))
        .unwrap();
    context.admit(&overflow).unwrap();
    assert_eq!(context.attempts.lock().unwrap().len(), 1024);
}

#[test]
fn login_input_bounds_precede_admission_and_do_not_change_existing_empty_input_semantics() {
    assert_eq!(LoginContext::validate_input("", ""), Ok(()));
    assert_eq!(
        LoginContext::validate_input(&"p".repeat(4096), &"0".repeat(64)),
        Ok(())
    );
    assert_eq!(
        LoginContext::validate_input(&"p".repeat(4097), "0"),
        Err(DurableAuthError::InvalidInput)
    );
    assert_eq!(
        LoginContext::validate_input("password", &"0".repeat(65)),
        Err(DurableAuthError::InvalidInput)
    );
}
