use crate::{fixture::*, offline::*, *};
#[tokio::test]
async fn chunk_http_configuration_is_inert_canonical_and_bounded() {
    let f = Offline::new().await;
    for origin in [
        ORIGIN,
        "https://127.0.0.1",
        "http://localhost",
        "http://127.0.0.1:3000",
    ] {
        assert!(FileChunkReadHttpState::new(f.source.clone(), f.workspace.clone(), origin).is_ok());
    }
    for origin in [
        "https://platform.example/",
        "http://192.168.1.9",
        "https://user@platform.example",
        "https://platform.example?x=1",
        "https://platform.example#x",
        "https://*.example",
        "",
    ] {
        assert_eq!(
            FileChunkReadHttpState::new(f.source.clone(), f.workspace.clone(), origin).err(),
            Some(TaskContentHttpConfigError::InvalidOrigin)
        );
    }
    for n in [0, 17, usize::MAX] {
        assert_eq!(
            FileChunkReadHttpState::with_capacity(f.source.clone(), f.workspace.clone(), ORIGIN, n)
                .err(),
            Some(TaskContentHttpConfigError::InvalidCapacity)
        );
    }
    for n in [1, 16] {
        assert!(FileChunkReadHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            ORIGIN,
            n
        )
        .is_ok());
    }
    available(f.app).await;
}
