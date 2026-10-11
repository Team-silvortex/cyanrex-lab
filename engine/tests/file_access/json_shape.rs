use super::{json_support::*, *};

#[test]
fn every_required_field_and_every_nested_object_is_strict() {
    for kind in [Kind::Publisher, Kind::Recipient] {
        let raw = kind.raw();
        for path in paths(&raw) {
            let keys: Vec<String> = at(&raw, &path)
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            for key in keys {
                let mut missing = raw.clone();
                at_mut(&mut missing, &path)
                    .as_object_mut()
                    .unwrap()
                    .remove(&key);
                kind.reject(&missing);
                let mut null = raw.clone();
                at_mut(&mut null, &path)[&key] = Value::Null;
                kind.reject(&null);
            }
            for key in [
                "role",
                "permissions",
                "path",
                "storage_path",
                "url",
                "download_url",
                "approved",
                "client_approved",
                "token",
                "grant_id",
            ] {
                let mut unknown = raw.clone();
                at_mut(&mut unknown, &path)[key] = json!({"allow":true});
                kind.reject(&unknown);
            }
        }
    }
}

#[test]
fn positional_arrays_are_not_object_records_at_any_depth() {
    for kind in [Kind::Publisher, Kind::Recipient] {
        let raw = kind.raw();
        for path in paths(&raw) {
            let mut array = raw.clone();
            *at_mut(&mut array, &path) = positional(at(&raw, &path));
            kind.reject(&array);
            for invalid in [json!([]), json!(true), json!("{}"), json!(1)] {
                let mut changed = raw.clone();
                *at_mut(&mut changed, &path) = invalid;
                kind.reject(&changed);
            }
        }
    }
}

#[test]
fn raw_duplicate_keys_remain_errors_even_when_values_match() {
    for kind in [Kind::Publisher, Kind::Recipient] {
        let raw = kind.raw();
        for path in paths(&raw) {
            let object = at(&raw, &path).as_object().unwrap();
            let encoded = serde_json::to_string(object).unwrap();
            for (key, value) in object {
                let duplicate = format!(
                    "{{{}:{},{}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string(value).unwrap(),
                    &encoded[1..]
                );
                let changed = substitute(&raw, &path, &duplicate);
                kind.reject_bytes(changed.as_bytes());
                let conflicting = format!(
                    "{{{}:null,{}",
                    serde_json::to_string(key).unwrap(),
                    &encoded[1..]
                );
                let changed = substitute(&raw, &path, &conflicting);
                kind.reject_bytes(changed.as_bytes());
            }
        }
    }
}

#[test]
fn direct_nested_deserialization_uses_the_same_strict_object_boundary() {
    let values = [
        serde_json::to_value(super::fixture::account(1)).unwrap(),
        serde_json::to_value(super::fixture::file()).unwrap(),
        serde_json::to_value(super::fixture::validity()).unwrap(),
    ];
    for (index, raw) in values.into_iter().enumerate() {
        let reject = |changed: Value| {
            let bad = match index {
                0 => serde_json::from_value::<FileAccountRef>(changed).is_err(),
                1 => serde_json::from_value::<FileVersionRef>(changed).is_err(),
                _ => serde_json::from_value::<FileGrantValidity>(changed).is_err(),
            };
            assert!(
                bad,
                "direct reference/validity must not bypass strict shape"
            );
        };
        for path in paths(&raw) {
            for key in at(&raw, &path).as_object().unwrap().keys() {
                let mut missing = raw.clone();
                at_mut(&mut missing, &path)
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                reject(missing);
                let mut null = raw.clone();
                at_mut(&mut null, &path)[key] = Value::Null;
                reject(null);
            }
            let mut unknown = raw.clone();
            at_mut(&mut unknown, &path)["approved"] = json!(true);
            reject(unknown);
            let mut array = raw.clone();
            *at_mut(&mut array, &path) = positional(at(&raw, &path));
            reject(array);
            let object = at(&raw, &path).as_object().unwrap();
            let encoded = serde_json::to_string(object).unwrap();
            for (key, value) in object {
                let duplicate = format!(
                    "{{{}:{},{}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string(value).unwrap(),
                    &encoded[1..]
                );
                let changed = substitute(&raw, &path, &duplicate);
                let bad = match index {
                    0 => serde_json::from_slice::<FileAccountRef>(changed.as_bytes()).is_err(),
                    1 => serde_json::from_slice::<FileVersionRef>(changed.as_bytes()).is_err(),
                    _ => serde_json::from_slice::<FileGrantValidity>(changed.as_bytes()).is_err(),
                };
                assert!(
                    bad,
                    "direct nested serde accepted duplicate at {path}/{key}"
                );
            }
        }
    }
}
