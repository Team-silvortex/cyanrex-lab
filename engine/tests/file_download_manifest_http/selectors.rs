use crate::{fixture::*, offline::*, support::*, *};

#[tokio::test]
async fn manifest_http_all_six_raw_fields_are_required_duplicates_and_unknown_authority_are_rejected(
) {
    let f = Offline::new().await;
    let good = uri();
    let query = good.split_once('?').unwrap().1;
    let fields = query.split('&').collect::<Vec<_>>();
    for omitted in 0..fields.len() {
        let missing = fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| (index != omitted).then_some(*field))
            .collect::<Vec<_>>()
            .join("&");
        rejected(
            &f,
            body_request(TOKEN, &format!("{ENDPOINT}?{missing}"), unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
        rejected(
            &f,
            body_request(TOKEN, &format!("{good}&{}", fields[omitted]), unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for changed in [
        ENDPOINT.to_owned(),
        format!("{ENDPOINT}?"),
        format!("{good}&"),
        format!("{ENDPOINT}?&{query}"),
        good.replace("&upload_id=", "&&upload_id="),
        good.replace("byte_length=17", "byte_length"),
        good.replace("byte_length=17", "byte_length="),
        good.replace("byte_length=17", "byte_length=17=17"),
        format!("{good}&chunk_index=0"),
        format!("{good}&owner=teacher"),
        format!("{good}&workspace_id={}", scope().workspace_id),
        format!("{good}&authority_id={}", authority()),
        format!("{good}&account_id={TOKEN}"),
        format!("{good}&token={TOKEN}"),
        format!("{good}&namespace=other"),
        format!("{good}&{}", "x".repeat(513)),
    ] {
        rejected(
            &f,
            body_request(TOKEN, &changed, unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
}

#[tokio::test]
async fn manifest_http_selectors_reject_lexical_aliases_and_check_empty_and_maximum_length() {
    let f = Offline::new().await;
    let good = uri();
    for key in ["grant_id", "upload_id", "file_id", "version_id"] {
        let value = good
            .split_once('?')
            .unwrap()
            .1
            .split('&')
            .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
            .unwrap();
        for alias in [
            value.to_uppercase(),
            value.replace('-', ""),
            "00000000-0000-0000-0000-000000000000".into(),
            format!("{{{value}}}"),
            format!("urn:uuid:{value}"),
            format!("+{value}"),
            value.replacen('0', "%30", 1),
        ] {
            if alias != value {
                rejected(
                    &f,
                    body_request(
                        TOKEN,
                        &good.replace(&format!("{key}={value}"), &format!("{key}={alias}")),
                        unread(),
                    ),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                )
                .await;
            }
        }
    }
    let sha = typed().expected_file.sha256().to_string();
    for alias in [
        sha.to_uppercase(),
        "a".repeat(63),
        "g".repeat(64),
        format!("+{sha}"),
    ] {
        if alias != sha {
            rejected(
                &f,
                body_request(TOKEN, &good.replace(&sha, &alias), unread()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
        }
    }
    for value in [
        "00",
        "01",
        "017",
        "-1",
        "+1",
        "1.0",
        "1e1",
        "%31",
        "18446744073709551616",
    ] {
        rejected(
            &f,
            body_request(
                TOKEN,
                &good.replace("byte_length=17", &format!("byte_length={value}")),
                unread(),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    rejected(
        &f,
        body_request(TOKEN, &good.replace("grant_id=", "%67rant_id="), unread()),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    rejected(
        &f,
        body_request(
            TOKEN,
            &good.replace(
                "byte_length=17",
                &format!("byte_length={}", MAX_FILE_TRANSFER_BYTES + 1),
            ),
            unread(),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    rejected(
        &f,
        body_request(
            TOKEN,
            &good.replace("byte_length=17", "byte_length=0"),
            unread(),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    for valid in [
        good.replace(
            "byte_length=17",
            &format!("byte_length={MAX_FILE_TRANSFER_BYTES}"),
        ),
        good.replace("byte_length=17", "byte_length=0")
            .replace(&sha, bytes::manifest(0, 0xab).sha256().as_str()),
    ] {
        error(
            &f.app,
            request(TOKEN, &valid),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
}
