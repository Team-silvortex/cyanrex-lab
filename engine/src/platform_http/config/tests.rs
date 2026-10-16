use super::{validate, TaskContentHttpConfigError as Error};

const ORIGIN: &str = "https://platform.example:8443";

#[test]
fn http_configuration_accepts_origin_only_and_every_bounded_capacity() {
    assert_eq!(validate(ORIGIN, &[]), Ok(()));
    for capacity in 1..=16 {
        assert_eq!(validate(ORIGIN, &[capacity]), Ok(()));
        assert_eq!(validate(ORIGIN, &[1, capacity, 16]), Ok(()));
    }
}

#[test]
fn http_configuration_rejects_invalid_capacity_in_every_position() {
    for invalid in [0, 17, usize::MAX] {
        for capacities in [
            &[invalid][..],
            &[invalid, 1][..],
            &[1, invalid][..],
            &[1, 4, invalid, 16][..],
        ] {
            assert_eq!(validate(ORIGIN, capacities), Err(Error::InvalidCapacity));
        }
    }
}

#[test]
fn http_configuration_origin_failure_precedes_empty_valid_or_invalid_capacity_lists() {
    for origin in ["null", "http://192.0.2.1", "https://platform.example/"] {
        for capacities in [&[][..], &[1, 16][..], &[0, 17, usize::MAX][..]] {
            assert_eq!(validate(origin, capacities), Err(Error::InvalidOrigin));
        }
    }
}
