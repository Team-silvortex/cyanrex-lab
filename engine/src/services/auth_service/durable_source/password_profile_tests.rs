use super::*;
use argon2::password_hash::{PasswordHash, SaltString};
use argon2::PasswordHasher;

const PARAMS: &str = "m=19456,t=2,p=1";

fn zero_b64(bytes: usize) -> String {
    "A".repeat((bytes * 8).div_ceil(6))
}

fn record(params: &str, salt: &str, digest: &str) -> String {
    format!("$argon2id$v=19${params}${salt}${digest}")
}

fn phc(params: &str) -> String {
    record(params, &zero_b64(8), &zero_b64(32))
}

fn invalid(encoded: &str) {
    // Never verify or derive attacker-selected profiles: this helper invokes pure validation only.
    assert_eq!(
        validate(encoded),
        Err(DurableAuthError::InvalidRecord),
        "{encoded:?}"
    );
}

#[test]
fn exactly_the_supported_parameters_are_accepted_in_any_order() {
    for params in [
        "m=19456,t=2,p=1",
        "m=19456,p=1,t=2",
        "t=2,m=19456,p=1",
        "t=2,p=1,m=19456",
        "p=1,m=19456,t=2",
        "p=1,t=2,m=19456",
    ] {
        assert_eq!(validate(&phc(params)), Ok(()), "{params}");
    }
}

#[test]
fn hostile_and_overflowing_costs_are_rejected_without_running_argon2() {
    for params in [
        "m=4294967295,t=2,p=1",
        "m=18446744073709551615,t=2,p=1",
        "m=18446744073709551616,t=2,p=1",
        "m=19456,t=4294967295,p=1",
        "m=19456,t=18446744073709551616,p=1",
        "m=19456,t=2,p=4294967295",
        "m=19456,t=2,p=536870912",
        "m=19456,t=2,p=18446744073709551616",
    ] {
        invalid(&phc(params));
    }
}

#[test]
fn syntactically_valid_but_different_cost_profiles_are_invalid_records() {
    for params in [
        "m=8,t=1,p=1",
        "m=19455,t=2,p=1",
        "m=19457,t=2,p=1",
        "m=19456,t=1,p=1",
        "m=19456,t=3,p=1",
        "m=19456,t=2,p=2",
        "m=0,t=2,p=1",
        "m=19456,t=0,p=1",
        "m=19456,t=2,p=0",
    ] {
        invalid(&phc(params));
    }
}

#[test]
fn duplicate_cost_keys_are_invalid_regardless_of_order_or_equal_values() {
    for params in [
        "m=19456,t=2,p=1,m=4294967295",
        "m=4294967295,m=19456,t=2,p=1",
        "m=19456,t=2,p=1,m=19456",
        "m=19456,t=2,p=1,t=4294967295",
        "t=4294967295,m=19456,t=2,p=1",
        "m=19456,t=2,p=1,t=2",
        "m=19456,t=2,p=1,p=4294967295",
        "p=4294967295,m=19456,t=2,p=1",
        "m=19456,t=2,p=1,p=1",
    ] {
        invalid(&phc(params));
    }
}

#[test]
fn missing_and_extra_parameters_do_not_gain_default_values_or_optional_semantics() {
    for params in [
        "t=2,p=1",
        "m=19456,p=1",
        "m=19456,t=2",
        "",
        "m=19456,t=2,p=1,keyid=YQ",
        "m=19456,t=2,p=1,data=YQ",
        "m=19456,t=2,p=1,x=1",
        "m=19456,t=2,p=1,v=19",
        "M=19456,t=2,p=1",
    ] {
        invalid(&phc(params));
    }
}

#[test]
fn costs_require_canonical_decimal_spelling_and_exact_parameter_structure() {
    for params in [
        "m=019456,t=2,p=1",
        "m=19456,t=02,p=1",
        "m=19456,t=2,p=01",
        "m=+19456,t=2,p=1",
        "m=-19456,t=2,p=1",
        "m=19456.0,t=2,p=1",
        "m=0x4c00,t=2,p=1",
        "m=1.9456e4,t=2,p=1",
        "m=19456,t=2,p=１",
        "m=19456, t=2,p=1",
        "m=19456,t=2,p=1 ",
        "m=19456,t=2,p=",
        "m==19456,t=2,p=1",
        "m=19456,,t=2,p=1",
        ",m=19456,t=2,p=1",
        "m=19456,t=2,p=1,",
    ] {
        invalid(&phc(params));
    }
}

#[test]
fn algorithm_and_explicit_version_must_match_exactly() {
    let valid = phc(PARAMS);
    for algorithm in [
        "argon2i",
        "argon2d",
        "Argon2id",
        "argon2id-extra",
        "scrypt",
        "",
    ] {
        invalid(&valid.replacen("argon2id", algorithm, 1));
    }
    invalid(&valid.replacen("v=19$", "", 1));
    for version in [
        "v=16",
        "v=0",
        "v=20",
        "v=019",
        "v=+19",
        "v=19,v=19",
        "v=4294967296",
    ] {
        invalid(&valid.replacen("v=19", version, 1));
    }
}

#[test]
fn salt_limits_count_decoded_bytes_without_requiring_uuid_text() {
    for length in [8, 9, 16, 32, 47, 48] {
        assert_eq!(
            validate(&record(PARAMS, &zero_b64(length), &zero_b64(32))),
            Ok(())
        );
    }
    for length in [0, 1, 2, 3, 7, 49, 64] {
        invalid(&record(PARAMS, &zero_b64(length), &zero_b64(32)));
    }
}

#[test]
fn phc_salt_characters_alone_do_not_establish_decodable_base64() {
    for salt in [
        "AAAAAAAAAA.",
        "AAAAAAAAAA-",
        "AAAAAAAAAAB",
        "AAAAAAAAAAAAA",
        "AAAAAAAAAAA=",
        "AAAAAAAAAA_",
        "AAAAAAAAAA ",
        "AAAAAAAAAA密码",
    ] {
        invalid(&record(PARAMS, salt, &zero_b64(32)));
    }
}

#[test]
fn digest_is_required_and_must_decode_to_exactly_thirty_two_bytes() {
    for length in [0, 1, 4, 16, 31, 33, 64, 65] {
        invalid(&record(PARAMS, &zero_b64(8), &zero_b64(length)));
    }
    let valid = phc(PARAMS);
    invalid(valid.rsplit_once('$').unwrap().0);
    for digest in [
        format!("{}B", "A".repeat(42)),
        format!("{}=", zero_b64(32)),
        "密码".into(),
    ] {
        invalid(&record(PARAMS, &zero_b64(8), &digest));
    }
}

#[test]
fn oversized_unicode_and_non_phc_shapes_are_invalid_records() {
    let valid = phc(PARAMS);
    for encoded in [
        "".into(),
        "a".repeat(64),
        "a".repeat(1024),
        "a".repeat(1025),
        "密码".repeat(180),
        format!("{valid}$extra"),
        format!("{valid}\n"),
        format!("{valid}\0"),
        format!(" {valid}"),
        phc(&format!("m={},t=2,p=1", "9".repeat(1025))),
    ] {
        invalid(&encoded);
    }
}

#[test]
fn durable_writer_emits_the_explicit_profile_with_the_original_salt_bytes() {
    let salt = "not-a-uuid-but-a-valid-salt";
    let encoded = derive("synthetic-profile-password", salt).unwrap();
    assert_eq!(validate(&encoded), Ok(()));
    let parsed = PasswordHash::new(&encoded).unwrap();
    assert_eq!(parsed.algorithm.as_str(), "argon2id");
    assert_eq!(parsed.version, Some(19));
    assert_eq!(parsed.params.as_str(), PARAMS);
    assert_eq!(parsed.hash.unwrap().as_bytes().len(), 32);
    let mut decoded = [0_u8; 48];
    assert_eq!(
        parsed.salt.unwrap().decode_b64(&mut decoded).unwrap(),
        salt.as_bytes()
    );
}

#[test]
fn existing_default_argon2id_records_remain_compatible_without_legacy_fallback() {
    let password = "synthetic-existing-default-password";
    let salt = SaltString::encode_b64(b"existing-default-salt").unwrap();
    let encoded = argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string();
    assert_eq!(validate(&encoded), Ok(()));
    assert_eq!(verify(password, &encoded), Ok(true));
    assert_eq!(verify("wrong-password", &encoded), Ok(false));
}

#[test]
fn missing_account_record_is_a_fixed_public_real_profile_hash() {
    let password = "cyanrex-public-missing-account-v1";
    assert_eq!(validate(MISSING_ACCOUNT_HASH), Ok(()));
    assert_eq!(
        derive(password, "cyanrex-public-missing-account").unwrap(),
        MISSING_ACCOUNT_HASH
    );
    // The dummy is intentionally matchable: login must still require a real account snapshot.
    assert_eq!(verify(password, MISSING_ACCOUNT_HASH), Ok(true));
    assert_eq!(verify("wrong-password", MISSING_ACCOUNT_HASH), Ok(false));
}
