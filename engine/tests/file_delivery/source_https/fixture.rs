use super::*;
use std::{future::Future, net::Ipv4Addr, pin::Pin};

pub const CHOSEN_LEAF: &str = "chosen-source-https.bin";
struct Peers(Mutex<Vec<Arc<SourceTlsServer>>>);
impl Peers {
    fn stop(&self) -> Vec<TlsObservation> {
        std::mem::take(&mut *self.0.lock().unwrap())
            .into_iter()
            .map(|peer| peer.finish())
            .collect()
    }
}
impl Drop for Peers {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
pub struct Fixture {
    // Stop/reap/join every owned TLS peer before the Source fixture's descriptors are dropped.
    peers: Peers,
    pub source: chunk_fixture::Fixture,
    pub certificates: Certificates,
}
impl Fixture {
    pub async fn new() -> Self {
        let source = chunk_fixture::Fixture::ready(1).await;
        let prepared = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let directory = source.sharing.base.directory.join("source-https-keys");
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            Certificates::generate(&directory)
        }));
        let certificates = match prepared {
            Ok(certificates) => certificates,
            Err(panic) => {
                source.cleanup().await;
                std::panic::resume_unwind(panic);
            }
        };
        Self {
            peers: Peers(Mutex::new(Vec::new())),
            source,
            certificates,
        }
    }
    pub fn token(&self) -> &str {
        &self.source.sharing.base.grants.auth.target_token
    }
    pub fn directory(&self, leaf: &str, mode: u32) -> PathBuf {
        let path = self.source.sharing.base.directory.join(leaf);
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }
    pub fn peer(&self, root: &PathBuf) -> Arc<SourceTlsServer> {
        self.peer_with_router(root).0
    }
    pub fn peer_with_router(&self, root: &PathBuf) -> (Arc<SourceTlsServer>, Router) {
        let router = cyanrex_engine::platform_http::build_file_distribution_router(
            cyanrex_engine::platform_http::FileDistributionHttpState::with_capacities(
                self.source.sharing.base.grants.auth.source.clone(),
                self.source.workspace.clone(),
                "https://platform.example",
                1,
                1,
            )
            .unwrap(),
        );
        let peer = Arc::new(SourceTlsServer::start(
            &self.certificates,
            router.clone(),
            root.clone(),
            Ipv4Addr::LOCALHOST,
        ));
        self.peers.0.lock().unwrap().push(peer.clone());
        (peer, router)
    }
    pub fn client(&self, peer: &SourceTlsServer, root: &PathBuf) -> FileChunkClient {
        FileChunkClient::with_config(
            &peer.origin,
            ChunkStaging::open(scope(), root, 1).unwrap(),
            FileChunkClientConfig {
                capacity: 1,
                trusted_root_pem: Some(self.certificates.ca.clone()),
                ..FileChunkClientConfig::default()
            },
        )
        .unwrap()
    }
    async fn cleanup(self) {
        let observations = self.peers.stop();
        // These scenarios stop at already-returned Source frames or before Source dispatch.
        // Draining the original auth pool after transport shutdown precedes schema/file removal.
        self.source.sharing.base.grants.auth.base.drain().await;
        let source_directory = self.source.sharing.base.directory.clone();
        self.source.cleanup().await;
        assert!(
            !source_directory.exists(),
            "actual Source schemas/files and stopped TLS keys are cleaned"
        );
        for observation in observations {
            assert_eq!(observation.error, None, "owned TLS peer exited cleanly");
        }
    }
}
pub async fn checked<T, F>(work: F) -> T
where
    F: for<'a> FnOnce(&'a Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let f = Fixture::new().await;
    let outcome = AssertUnwindSafe(work(&f)).catch_unwind().await;
    f.cleanup().await;
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
