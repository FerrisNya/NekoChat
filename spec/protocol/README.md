# NekoChat protocol, version 1 (draft)

**Status:** draft for review. Nothing here is implemented yet. Phase 1 of the [roadmap](../roadmap.md) builds the first version.

This spec defines what NekoChat nodes say to each other: the records they sign, the messages they send and the rules for accepting them. It doesn't define the client API between the web app and its own node; that's Phase 1 step 2.

The [decisions](../decisions.md) it builds on: Iroh for transport, Ed25519 root and device keys, MLS for group encryption, SQLite for storage.

## Documents

| Document | Covers |
|---|---|
| [01-identity.md](01-identity.md) | User IDs, root and device keys, the identity log, profiles, handles, recovery, instance sign-in (including Google, Discord and other providers), linked accounts |
| [02-transport.md](02-transport.md) | Iroh connections, framing, the session handshake, the list of requests |
| [03-contacts-and-mailboxes.md](03-contacts-and-mailboxes.md) | Reachability records, contact requests, consent, mailboxes, delivery tokens, blocking |
| [04-messaging.md](04-messaging.md) | Envelopes, MLS groups, message content, ordering, edits, ephemeral events |
| [05-blobs.md](05-blobs.md) | Encrypted file transfer, attachment descriptors, retention |
| [06-communities.md](06-communities.md) | Home nodes, the community log, permissions, invites, encrypted channels, migration |
| [07-discovery-and-federation.md](07-discovery-and-federation.md) | Instance records, directories, gossip, opt-out, defederation, finding people by linked account or area, people nearby on the same network |

## The pieces

```
 ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
 │  Light node  │      │ Desktop app  │      │   Instance   │  ← all run neko_node
 │ (localhost UI)│      │ (embedded)   │      │ (web UI, many│
 └──────┬───────┘      └──────┬───────┘      │   accounts)  │
        │                     │              └──────┬───────┘
        └────── Iroh (QUIC, dial by key) ───────────┘
                     │                  │
              direct connection     or relay
```

- **Node:** one running `neko_node`. It has one Iroh endpoint per device key it serves. A light node or desktop app serves one device. An instance also serves its own server key, plus a device key for each account's browser sessions.
- **Instance:** a node that hosts accounts, serves the web UI, and offers mailboxes and relays to its members.
- **Home node:** the node that orders a community's changes and messages. It can be an instance or a member's own node.
- **Mailbox:** store-and-forward storage that holds envelopes for a user while their devices are offline.
- **Relay:** an Iroh relay server. It forwards encrypted QUIC traffic when two nodes can't connect directly. Relays never see message contents.

## Conventions

The key words MUST, SHOULD and MAY follow RFC 2119.

### Encoding

- All records and messages are **CBOR** (RFC 8949), encoded with the **core deterministic encoding** rules (RFC 8949 §4.2.1): shortest integer forms, definite lengths, map keys sorted by their encoded bytes.
- Map keys are short text strings.
- Schemas are written in **CDDL** (RFC 8610).
- Inside MLS, messages use MLS's own TLS presentation-language encoding. The CBOR envelope carries them as byte strings.

Shared CDDL types used throughout:

```cddl
; 32-byte Ed25519 public key
pubkey = bstr .size 32
; 64-byte Ed25519 signature
sig = bstr .size 64
; 32-byte BLAKE3 hash
hash = bstr .size 32
; Milliseconds since the Unix epoch, UTC
timestamp = uint
; A user's permanent ID (01-identity.md)
user-id = hash
; A device's Ed25519 public key, which is also its Iroh node ID
device-id = pubkey
; A community's permanent ID (06-communities.md)
community-id = hash
; 16-byte time-ordered ID (below)
neko-id = bstr .size 16
```

### Signed records

A signed record is a CBOR map with this shape:

```cddl
signed = {
  "t": tstr,          ; record type, e.g. "identity.device"
  "v": uint,          ; format version of that record type
  "body": bstr,       ; deterministic CBOR of the type's body map
  "by": pubkey,       ; signing key
  "sig": sig,
}
```

The signature covers:

```
"NekoChat-v1:" ‖ t ‖ 0x00 ‖ uint64-be(v) ‖ body
```

Prefixing the type stops a signature for one kind of record being replayed as another.

Verifiers MUST check the signature over the exact `body` bytes received. They MUST NOT re-encode the body first. Nodes store and forward the original bytes.

The **record hash** is `BLAKE3(deterministic CBOR of the whole signed map)`. Records refer to each other by this hash.

### Hashes and IDs

- **Hash function:** BLAKE3, 32-byte output. Iroh blobs use the same hash.
- **neko-id:** a 16-byte, time-ordered ID used for messages, channels and other objects with no natural hash.
  - Layout: 48-bit big-endian Unix milliseconds, then 80 random bits (the same layout as a ULID).
  - IDs sort roughly by creation time, and the creation time can be read from the first 6 bytes.
  - Collisions are negligible without any coordination: 80 random bits per millisecond.
- **Text form** of keys, hashes and IDs: lowercase RFC 4648 base32 without padding. Iroh shows node IDs the same way.

### Time

- Timestamps come from the sender's clock and are informational.
- Receivers MUST accept timestamps up to 10 minutes in the future and SHOULD flag anything further ahead.
- Ordering never depends on timestamps where an authority provides a sequence number (community channels, [06](06-communities.md)).

### Versioning

- **Protocol major version:** carried in the Iroh ALPN, `nekochat/1`. A breaking change to the wire protocol means a new ALPN (`nekochat/2`). Nodes MAY offer both during a migration, and the handshake picks the highest version both support.
- **Record versions:** each record type carries its own `v`. Adding optional fields doesn't change `v`. Removing a field, changing a field's meaning or adding a required field does.
- **Unknown fields:** receivers MUST ignore map keys they don't recognise. They MUST keep the original bytes, so signatures still verify when the record is forwarded.
- **Unknown record types or versions:** ignored, not rejected, unless the containing message says the record is required.
- Every request and record type in this spec is version 1.

### Limits

| Item | Limit |
|---|---|
| One framed message on a stream | 1 MiB |
| Envelope `payload` | 256 KiB |
| Text in a message body | 16 000 UTF-8 bytes |
| Contact request note | 500 UTF-8 bytes |
| Inline attachment thumbnail | 16 KiB |
| Attachments per message | 10 |
| Devices per user | 32 |
| Mailbox retention | 30 days, or until every device has acknowledged |

Nodes MAY set lower limits for what they store for others (mailboxes, home nodes). They MUST accept anything within these limits from their own user's devices.

## Open questions

These are the questions this draft leaves open. Each document's own Open questions section has the details.

1. **Channel history for new members.** MLS doesn't give new members past messages. Options include sharing an encrypted history key on join, or having the home node keep a history copy for unencrypted channels only. ([04](04-messaging.md), [06](06-communities.md))
2. **Hiding the sender from mailbox hosts.** Mailboxes currently learn which node deposited each envelope. Sealed-sender tokens could hide that. ([03](03-contacts-and-mailboxes.md))
3. **Commit conflicts in DMs.** With no ordering authority, two devices can change a DM group at the same moment. The draft uses a lowest-hash-wins rule; this needs testing. ([04](04-messaging.md))
4. **Standby home nodes.** Whether a community can name a backup home that takes over automatically when its home is offline. ([06](06-communities.md))
5. **Spam controls for contact requests.** The draft uses rate limits. It doesn't yet say whether to add proof-of-work or introductions through mutual contacts. ([03](03-contacts-and-mailboxes.md))
6. **Recovery window length.** The draft gives a recovery key 72 hours to override a root-key change. ([01](01-identity.md))
7. **Permission model.** Communities reuse Fluxer's permission bits. The exact list is fixed when Phase 5 ports them. ([06](06-communities.md))
8. **Message IDs in the web app.** The app assumes 64-bit snowflake IDs in about 52 files. 16-byte neko-ids need a client-side change ([cut-lines §11](../cut-lines.md)). ([04](04-messaging.md))
9. **Passkey coverage.** Protecting the root key for accounts that sign in with Google or Discord relies on the WebAuthn `prf` extension, which not every browser supports yet. ([01](01-identity.md))
10. **Fake clients in area search.** Per-client rate limits are weak against someone running many clients. Directories could require a token from the searcher's instance, which would leave light-node users out. ([07](07-discovery-and-federation.md))
