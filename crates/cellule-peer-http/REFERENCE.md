> Detailed reference preserved from the original Cellule synthesis.
> The [current crate guide](README.md) is the entry point; Crab product
> examples below describe the former embedding service.

# Crab Cell peer HTTP transport

`PeerHttpRoundTrip` sends authenticated Cell peer envelopes to the current
enrolled owner. It reloads the authority and node directory on a retry, bounds
responses, distinguishes an unknown outcome from a request that never started,
and caches HTTP clients by enrolled session, certificate, and public key.

Owner-routed requests make at most two attempts. On HTTP 429/503 with an integer
`Retry-After` header, the transport paces the retry within the original deadline,
then reloads ownership. Persistent admission rejection returns a capacity error.
A delay that leaves no request budget returns a deadline error without sleeping.
Responses without that header retain the immediate owner-refresh behavior used
by existing receivers. Lost or invalid responses remain unknown outcomes and
are not retried here. Direct-node activation remains a single attempt: its
caller owns scheduling and retries, and receives the same capacity distinction.

The product supplies `PeerTargetScope` and `PeerHttpClientFactory`. The factory
must authenticate its local client identity and pin the remote certificate and
public key passed to it. The receiver must verify the peer envelope, enrollment,
and product authorization before dispatching through `PeerDispatcher`.

The crate provides no public ingress or credential discovery. Install a receiver
at `internal/cells/v1/forward` only after the embedding service has wired its
identity, enrollment, authorization, and readiness checks. Both owner-routed
and direct-node requests reject bodies above `MAX_PEER_REQUEST_BYTES` and a
zero deadline before provider lookup or dispatch.

`cargo test -p cellule-peer-http --locked` covers both routes' admission bounds,
real local HTTP retry/unknown-outcome classification, and TLS fleet identity
canonicalization. The HTTP fixtures do not qualify deployed mTLS enrollment or
certificate rotation; those require the embedding service's fleet tests.
