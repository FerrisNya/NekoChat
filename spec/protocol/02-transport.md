# 02. Transport

Nodes talk over Iroh: QUIC connections addressed by Ed25519 public key, with direct paths through home routers and relays as fallback. This document defines what runs over those connections.

The protocol above this layer only assumes "an authenticated, reliable, ordered byte stream to device X". A later WebRTC transport for browsers can carry the same frames ([decisions D1](../decisions.md)).

## Endpoints and addresses

- Each device key is an Iroh node ID. A node serving several device keys runs one Iroh endpoint per key.
- A device's **dial info** is its Iroh `NodeAddr`: its node ID, its home relay URL, and any direct addresses it knows.
- Nodes publish their dial info through Iroh's discovery (DNS and the Mainline DHT, using pkarr). They also give it directly to contacts in contact cards ([03](03-contacts-and-mailboxes.md)).
- **Relays:** each node is configured with one or more relay URLs: its instance's relay, a friend's, or a public one. Iroh picks one as the home relay and falls back between them. Losing every relay only blocks peers that can't connect directly; direct paths keep working.

## Protocols (ALPN)

| ALPN | Purpose |
|---|---|
| `nekochat/1` | Everything in this spec: requests, deliveries, subscriptions |
| The ALPN defined by the `iroh-blobs` version in use | Blob transfer ([05](05-blobs.md)) |

A node MUST refuse ALPNs it doesn't offer.

## Framing

Every exchange runs on its own bidirectional QUIC stream. Each frame is:

```
varint length (QUIC variable-length integer) ‖ CBOR value
```

A frame MUST NOT exceed 1 MiB. A node that receives a larger one MUST reset the stream.

### Requests and responses

```cddl
request = {
  "m": tstr,          ; method name, e.g. "mailbox.deposit"
  "p": any,           ; method parameters
}

response = ok-response / err-response
ok-response = { "ok": any }
err-response = { "err": { "code": err-code, ? "msg": tstr } }

err-code = "unauthorized" / "forbidden" / "not_found" / "rate_limited" /
           "too_large" / "invalid" / "unsupported" / "unavailable" / "internal"
```

- The opener sends one request and closes its send side. The other side sends one response and closes.
- **Subscriptions** (`*.subscribe`) are the exception: after the `ok` response, the responder keeps sending event frames until either side closes the stream.

## Session handshake

The first stream on every connection is the handshake. Iroh's TLS has already proved the remote node ID; the handshake says *whose* device it is.

```cddl
hello = {
  "versions": [+ uint],            ; protocol versions offered, e.g. [1]
  ? "user": user-id,               ; omitted by anonymous peers
  ? "head": hash,                  ; sender's identity head
  ? "device-cert": bytes,          ; sender's identity.device record
  "role": "device" / "instance",
  ? "instance": tstr,              ; domain, when role = "instance"
}
```

Method `hello`. The response is a `hello` from the other side.

The receiver checks that:
1. the `device-cert` names the Iroh-authenticated remote node ID
2. the cert chains to `user`, fetching the identity bundle with `identity.get` if the cached head differs ([01](01-identity.md))

If either check fails, the connection is treated as **anonymous**.

Instances connect to each other with `role: "instance"`, authenticated by their server key ([07](07-discovery-and-federation.md)).

## Who can call what

Access depends on what the caller has proved:

| Caller | Allowed |
|---|---|
| **Anonymous** | `hello`, `identity.get`, `reach.get`, `mailbox.deposit` (contact requests only), `instance.info` |
| **Identified stranger** (valid user, not a contact) | The above, plus `mailbox.deposit` with a valid delivery token |
| **Contact** | The above, plus `deliver`, `profile.get`, `keypackage.get`, `presence.subscribe` |
| **Community member** (on the community's home node) | `community.*` requests for that community, within their permissions |
| **The user's own device** | Everything, including `mailbox.fetch`, `mailbox.ack`, `device.sync` |

Nodes MUST rate-limit by remote node ID and by user ID. They SHOULD close connections that only send failing requests.

## Method index

Each document defines its own methods:

| Method | Defined in |
|---|---|
| `hello` | this document |
| `identity.get`, `profile.get` | [01](01-identity.md) |
| `reach.get`, `mailbox.deposit`, `mailbox.fetch`, `mailbox.ack`, `mailbox.subscribe`, `mailbox.revocations.put` | [03](03-contacts-and-mailboxes.md) |
| `deliver`, `keypackage.get`, `presence.subscribe`, `device.sync` | [04](04-messaging.md) |
| `community.*` | [06](06-communities.md) |
| `account.blob.pin`, `account.blob.unpin` (own instance only) | [05](05-blobs.md) |
| `instance.info`, `discovery.peers` | [07](07-discovery-and-federation.md) |

`identity.get`:

```cddl
identity-get-params = { "user": user-id, ? "since": hash }
identity-get-result = { "records": [* bytes] }   ; signed records after "since", oldest first
```

## Offline and reconnecting

- No connection is assumed to last. Every request can be retried. Requests with side effects carry an ID (a `neko-id` or a record hash), so retries are idempotent.
- When a device can't reach a peer directly, it falls back to that peer's mailbox ([03](03-contacts-and-mailboxes.md)). Nothing is lost while a relay or the peer is down; it waits in the mailbox or in the sender's outbox.

## Open questions

- **Idle connections:** how long to keep them open, and whether to keep a warm connection to each contact's devices (faster delivery, more battery use on mobile).
