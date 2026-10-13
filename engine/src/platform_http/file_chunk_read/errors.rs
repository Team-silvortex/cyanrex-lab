use crate::{
    platform_http::errors::HttpError,
    services::auth_service::durable_source::SessionFileChunkReadError,
};
use axum::http::StatusCode;

/// Service dispatch never establishes delivery, rollback or a fresh authorization at body polling.
pub(super) fn source(error: SessionFileChunkReadError) -> HttpError {
    use SessionFileChunkReadError as C;
    use StatusCode as S;
    let (status, code) = match error {
        C::Metadata(error) => return error.into(),
        C::InvalidInput => (S::BAD_REQUEST, "invalid_request"),
        C::CapacityExhausted => (S::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        C::Unavailable => (S::SERVICE_UNAVAILABLE, "service_unavailable"),
    };
    HttpError::dispatched(status, code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform_http::errors::file_download_tests::{
        assert_error, assert_projection_matrix,
    };

    #[tokio::test]
    async fn metadata_projection_keeps_complete_private_download_error_matrix() {
        assert_projection_matrix(|error| source(SessionFileChunkReadError::Metadata(error))).await;
    }

    #[tokio::test]
    async fn chunk_specific_input_capacity_and_unavailable_errors_remain_distinct() {
        for (error, status, code) in [
            (
                SessionFileChunkReadError::InvalidInput,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                SessionFileChunkReadError::CapacityExhausted,
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
            ),
            (
                SessionFileChunkReadError::Unavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            ),
        ] {
            assert_error(source(error), status, code).await;
        }
    }
}
