use super::*;
use axum::{Router, body::Body, response::Response, routing::post};
use cellule_runtime::identity::{ApplicationId, NamespaceId, NodeId, TenantId};
use cellule_runtime::ltx::CellStorageLayout;
use cellule_runtime::node::{NodeCapacity, NodeFailureDomain};
use cellule_store::Store;
use ed25519_dalek::SigningKey;
use object_store::{memory::InMemory, path::Path};

struct TestClients;

impl PeerHttpClientFactory for TestClients {
    fn client(&self, _: Digest, _: [u8; 32]) -> cellule_runtime::Result<reqwest::Client> {
        // HTTP fixtures exercise the round-trip classification only. Production
        // identity pinning is the responsibility of PeerTlsClient.
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(peer_transport)
    }
}

fn fixture() -> (PeerHttpRoundTrip, CellTarget, NodeAdvertisement) {
    let application = ApplicationId::from_bytes([1; 16]);
    let tenant = TenantId::from_bytes([2; 16]);
    let digest = Digest::from_bytes([3; 32]);
    let layout = CellStorageLayout::new(
        Store::new(Arc::new(InMemory::new())),
        Path::from("peer-tests"),
        *application.as_bytes(),
    );
    let transport = PeerHttpRoundTrip::new(
        Arc::new(ApplicationIdentity::new(tenant, application)),
        CellAuthority::new(layout.clone()),
        NodeDirectory::new(layout, digest, digest, digest),
        Arc::new(TestClients),
        SessionId::from_bytes([4; 16]),
    );
    let target =
        CellTarget::new(tenant, application, NamespaceId::from_bytes([5; 16]), b"p").unwrap();
    let now = now_ms().unwrap();
    let node = NodeAdvertisement::sign(
        NodeId::from_bytes([6; 16]),
        SessionId::from_bytes([7; 16]),
        "https://peer.test:443".into(),
        digest,
        digest,
        digest,
        digest,
        &SigningKey::from_bytes(&[8; 32]),
        1,
        now,
        now + 10_000,
        vec![digest],
        vec![1],
        NodeFailureDomain::default(),
        NodeCapacity::default(),
    )
    .unwrap();
    (transport, target, node)
}

#[tokio::test]
async fn both_routes_reject_oversized_requests_before_dispatch() {
    let (transport, target, node) = fixture();
    for direct in [false, true] {
        let request = vec![0; cellule_runtime::peer::MAX_PEER_REQUEST_BYTES + 1];
        let result = if direct {
            transport
                .send_to_node(target.clone(), node.clone(), request, 1_000)
                .await
        } else {
            transport.send(target.clone(), request, 1_000).await
        };
        assert!(
            matches!(
                result,
                Err(CellError::Peer("request exceeds peer byte limit"))
            ),
            "direct={direct}"
        );
    }
}

#[tokio::test]
async fn both_routes_reject_exhausted_deadlines_before_dispatch() {
    let (transport, target, node) = fixture();
    for direct in [false, true] {
        let result = if direct {
            transport
                .send_to_node(target.clone(), node.clone(), vec![], 0)
                .await
        } else {
            transport.send(target.clone(), vec![], 0).await
        };
        assert!(
            matches!(result, Err(CellError::Deadline)),
            "direct={direct}"
        );
    }
}

async fn http_attempt(status: StatusCode, delay: Option<&'static str>) -> PeerHttpAttempt {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/internal/cells/v1/forward",
        post(move || async move {
            let mut response = Response::builder().status(status);
            if let Some(delay) = delay {
                response = response.header(header::RETRY_AFTER, delay);
            }
            response.body(Body::from("fixture response")).unwrap()
        }),
    );
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    let (transport, _, _) = fixture();
    let peer = RemotePeer {
        session: SessionId::from_bytes([9; 16]),
        endpoint: format!("http://{address}/").parse().unwrap(),
        certificate: Digest::from_bytes([10; 32]),
        public_key: [11; 32],
    };
    let result = transport.send_once(&peer, vec![1], 5_000).await;
    stop.send(()).unwrap();
    server.await.unwrap();
    result.unwrap()
}

#[tokio::test]
async fn admission_responses_preserve_retry_delay() {
    for status in [
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        assert!(matches!(
            http_attempt(status, Some("2")).await,
            PeerHttpAttempt::Retry(CellError::Capacity(_), delay) if delay == Duration::from_secs(2)
        ));
    }
}

#[tokio::test]
async fn server_failure_or_invalid_success_remains_unknown() {
    for status in [StatusCode::INTERNAL_SERVER_ERROR, StatusCode::OK] {
        assert!(matches!(
            http_attempt(status, None).await,
            PeerHttpAttempt::Unknown(_)
        ));
    }
}
