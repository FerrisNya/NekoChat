# Decisions

Choices that the rest of the spec builds on. Changing one means revisiting every document that depends on it.

## D1. Transport: Iroh

**Decided:** 2026-10-27. **Roadmap:** Phase 0 step 2.

Nodes connect with [Iroh](https://iroh.computer): QUIC connections dialled by public key, with direct connections through home routers and automatic fallback to relays. A node's Iroh node ID is its device key (see D2).

- **Why:** dialling by key matches the identity model, so a node can reach known contacts without a central server. Relays are simple to self-host, which fits "a node can use several relays". `iroh-blobs` covers content-addressed file transfer (Phase 3).
- **Rejected:** rust-libp2p. It's more flexible and has a general-purpose DHT and browser WebRTC, but it's more code to assemble and tune. NekoChat's discovery uses directories and gossip, not DHT search.
- **Known gap:** browsers can't send raw UDP, so a browser-only client reaches other nodes through relays. Direct browser connections need a separate WebRTC transport later. [protocol/02-transport.md](protocol/02-transport.md) keeps the protocol independent of Iroh so that can be added.

## D2. Identity: Ed25519 root key plus signed device keys

**Decided:** 2026-10-27.

Each user has a long-term Ed25519 root key. Each device has its own Ed25519 key, certified by the root key. Losing a device means revoking one certificate, not replacing the identity. An optional recovery key can replace a compromised root key.

- **Why:** it works the same for a light node, the desktop app and an instance account, and identities can move between instances.
- **Rejected:**
  - One key copied to every device: no per-device revocation.
  - Instance-bound accounts (`@user@server` as the identity): light nodes have no server, and moving instances changes the identity.
  - `did:plc`: depends on a central registry.

## D3. Group encryption: OpenMLS

**Decided:** 2026-10-27.

End-to-end encryption uses MLS (RFC 9420) through [OpenMLS](https://openmls.tech). DMs are two-member MLS groups, so there is one encryption system, not two.

- **Why:** membership changes cost grows logarithmically with group size, and groups recover security after a key is stolen. MLS needs one authority to order membership changes, and a community's home node already provides that.
- **Rejected:**
  - Signal-style sender keys: membership changes cost more as groups grow, and group recovery after compromise is weak.
  - Matrix Olm/Megolm: tolerant of having no ordering, but weak recovery after compromise. Matrix itself is moving towards MLS.
- **Cost:** people who join a channel can't read its earlier messages until a history-sharing step exists ([protocol/README.md](protocol/README.md#open-questions)).

## D4. Storage: SQLite, with Postgres as an option for instances

**Decided:** 2026-10-27.

Every node stores its data in SQLite. Full-text search uses SQLite's FTS5, and SQLCipher can encrypt the file at rest. Instances may use Postgres when they need many concurrent writers. OpenMLS state lives in the same database through its storage interface.

- **Why:** one file, no server, and it runs on desktop, mobile, a light node and (through WebAssembly) a browser.
- **Rejected:**
  - Postgres everywhere: impossible on a light node.
  - Embedded key-value stores (redb, sled, RocksDB): every index and query would have to be built by hand.
