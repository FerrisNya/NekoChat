# NekoChat roadmap

Replace Fluxer's centralized messaging/upload backend with a hybrid P2P + federated network, keeping the Fluxer-derived frontend. Each phase ends with something runnable; nothing is deleted until its replacement works.

## Phase 0: Groundwork (1–2 weeks)
1. ✅ **Rebrand the identifiers.** Packages, bundle ID, update/push endpoints, Docker image names. See [rebrand-placeholders.md](rebrand-placeholders.md). Internal code names are deferred to Phase 9.
2. **Lock in the main choices:** transport (Iroh or rust-libp2p), identity (Ed25519 keys, cross-signed devices), group encryption (OpenMLS), local storage (SQLite).
3. ✅ **Map the cut lines.** See [cut-lines.md](cut-lines.md). The Decision column is still to be filled in.
4. **Write a protocol spec** in `spec/`: identity records, contact requests, message envelopes, blobs, community authority, discovery records, all with versioned wire formats.

**Done when** the spec is reviewed and the repo builds under the NekoChat name.

## Phase 1: Node skeleton (3–4 weeks)
1. New `neko_node/` Rust crate in the Cargo workspace: identity and keystore, SQLite store, transport (direct, relay, hole-punching, LAN discovery).
2. **Client API**: REST plus an event stream mirroring the existing request/event shapes, so the UI barely changes.
3. Ship `nekochat-node`, which serves the built web app on `localhost`.

**Done when** two light nodes on different networks can see each other by key.

## Phase 2: Consent and DMs (3–4 weeks)
1. Contact requests via mailbox; accept, reject, block. Nothing is shared until accepted.
2. MLS-encrypted 1:1 messages, queued while the peer is unreachable; multiple relays per node.
3. **Frontend:** node-transport switch behind `RestTransport.ts`, `InstanceHTTP.ts`, `GatewaySocket.ts`; screens for keys, devices, contact requests.

**Done when** two people can DM through the real UI, survive a network change, and recover after a relay outage.

## Phase 3: Files and media (2–3 weeks)
1. Content-addressed, encrypted blobs: upload = store locally; download = fetch from sender's node or home instance.
2. Thumbnails and link previews move into client/node, reusing `fluxer_unfurl` and parts of `fluxer_media_proxy` as libraries.

**Done when** attachments, images and embeds work in DMs without the old upload pipeline.

## Phase 4: Instance mode (4–6 weeks)
1. `neko-node --instance`: public web UI hosting, account login that unlocks the user's keys, mailbox/relay service for members. Postgres optional.
2. **Federation:** instance-to-instance over HTTPS/QUIC, signed server keys, defederation and block lists.
3. Browser client uses the instance as its home node.

**Done when** users on instance A, instance B and a light node can all DM each other.

## Phase 5: Communities (6–8 weeks, biggest phase)
1. One authoritative home node per guild: ordering, roles/permissions, invites. Members keep replicas.
2. Port guild/channel/member/permission logic (not infrastructure) from `fluxer_api` and `fluxer_gateway` into the node.
3. Optional per-channel MLS per community.

**Done when** instance-hosted and P2P-hosted communities both work across federation.

## Phase 6: Discovery (2–3 weeks)
1. Instances announce to a default directory and gossip known instances; P2P nodes enter the DHT only on opt-in.
2. Discovery UI that respects opt-out and defederation.

## Phase 7: Desktop and voice (3–4 weeks)
1. Embed `neko_node` in the desktop app as a native addon; point the module updater and Velopack at NekoChat's release feeds.
2. **Voice:** direct WebRTC for 1:1 and small calls; LiveKit on instances that run it.

## Phase 8: Remove the old backend
Delete Fluxer services in dependency order once nothing calls them:
1. `fluxer_messages`, `fluxer_users`, `fluxer_snowflakes`, `fluxer_push`.
2. Message, upload and gateway parts of `fluxer_api`, plus billing/Stripe.
3. `fluxer_gateway`.
4. NATS, Valkey, Cassandra, SeaweedFS from `deploy/` and `.devcontainer/`.

Then update CI, docs and the Helm chart.

## Phase 9: Internal rename (1–2 weeks)
Rename the Fluxer code names that survive Phase 8, so nothing implies the project tracks Fluxer upstream. Done after the cut so deleted code isn't renamed and the removal diff stays reviewable.

1. **Inventory survivors.** Re-run the counts against what's left (before the cut: ~7,500 `@fluxer/*` refs in ~2,800 files, `FLUXER_*` in ~340 files, 17 `fluxer_*` folders).
2. **Packages:** `@fluxer/*` → `@nekochat/*` (package.json names, imports, tsconfig paths, Rspack/vitest aliases, pnpm workspace).
3. **Folders:** `fluxer_*` → the new layout (e.g. `apps/web`, `apps/desktop`, `node/`, `packages/*`, or `nekochat_*`), via `git mv`. Update the Cargo workspace, Dockerfiles, compose, Helm, CI path filters, CODEOWNERS.
4. **Rust crates and Erlang/other modules** that keep `fluxer` in their names.
5. **Environment variables:** `FLUXER_*` → `NEKOCHAT_*`. No compatibility aliases needed; there are no existing deployments.
6. **Remaining user-facing "Fluxer" strings** in UI text and translations (~400), as a translation pass.
7. **Docs:** rename or rewrite `fluxer_docs`, the README, and the desktop identity leftovers (`FluxerAudioIdentity.ts`, provisioning profile names).
8. **Keep attribution (AGPL-3.0):** LICENSE, existing "Fluxer Platform AB" copyright notices, and a "derived from Fluxer" credit stay.

Rules: rename-only commits (no behaviour changes mixed in) so `git log --follow` and blame trace through; build and full test suite green after each commit.

**Done when** `git grep -i fluxer` only hits attribution, license notices, and historical changelog entries.

## Throughout
- **Testing:** multi-node/relay/instance harness behind simulated NAT, in CI from Phase 1.
- **Security:** review of key handling and encryption before any public release.
- **Mobile:** after Phase 7; node code compiles for Android/iOS via UniFFI.

Rough total: **8–12 months** for one experienced developer. Phase 5 carries the most risk.
