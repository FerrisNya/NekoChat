# 01. Identity

A user is a chain of signed records that starts with a genesis record. Devices act for the user with their own keys, certified by the user's root key. Nothing here depends on any server: a light node can create and verify identities offline.

## Keys

| Key | Type | Held by | Used for |
|---|---|---|---|
| **Root key** | Ed25519 | One "primary" device, or the home instance encrypted with the user's password, or offline | Certifying and revoking devices, rotating itself |
| **Recovery key** | Ed25519, optional | Offline: a recovery phrase or paper backup | Replacing a lost or stolen root key |
| **Device key** | Ed25519 | One device, never copied | Iroh node ID, signing messages and records, MLS leaf signature key |
| **Contact key** | X25519 | Every device of the user (shared) | Receiving sealed contact requests ([03](03-contacts-and-mailboxes.md)) |

- A device key MUST NOT be reused across users.
- The root key SHOULD NOT live on every device. A device only needs the root key while it certifies a new device or revokes one.
- Instance accounts: the instance holds a device key for the account's browser sessions. It MAY also hold the root key, encrypted with a key derived from the user's password using Argon2id. Logging in decrypts it in the browser, so the instance never sees the plain root key. (Phase 4, "account login unlocks keys".)
- The contact key is the one key shared between devices. It only decrypts contact requests, never messages. It is replaced whenever a device is revoked.

## User ID

```
user-id = BLAKE3(record bytes of the genesis record)
```

The user ID never changes, even when the root key rotates. In text it's base32, for example `nk:3vq7…` (the `nk:` prefix is display-only).

## The identity log

A user's identity is an append-only log of signed records. Each record after the genesis names the previous one by hash.

### identity.genesis

```cddl
genesis-body = {
  "root": pubkey,
  ? "recovery": pubkey,
  "created": timestamp,
  "nonce": bstr .size 16,     ; makes every genesis unique
}
```

Signed by `root`.

### identity.rotate

Replaces the root key.

```cddl
rotate-body = {
  "user": user-id,
  "prev": hash,               ; hash of the latest log record
  "root": pubkey,             ; the new root key
  ? "recovery": pubkey,       ; a new recovery key, if changing it
  "at": timestamp,
}
```

Signed by the current root key or by the recovery key.

**Recovery window:** a rotation signed by the recovery key overrides any root-signed rotation made in the 72 hours before it.

- This lets the owner take an identity back after someone steals the root key and rotates it.
- When a log forks, verifiers keep the branch whose first diverging record is recovery-signed, if it falls inside the window. Otherwise they keep the earliest valid branch.
- Records on the discarded branch become invalid, including device certificates.

### identity.device

Certifies a device.

```cddl
device-body = {
  "user": user-id,
  "prev": hash,
  "device": device-id,
  "name": tstr .size (0..64),  ; e.g. "Nyaeko's laptop", shown to contacts
  "kind": "light" / "desktop" / "instance" / "mobile",
  "created": timestamp,
  ? "expires": timestamp,
}
```

Signed by the current root key.

### identity.revoke

```cddl
revoke-body = {
  "user": user-id,
  "prev": hash,
  "device": device-id,
  "at": timestamp,
  "reason": "lost" / "compromised" / "retired",
}
```

Signed by the current root key.

- After `at`, nothing signed by that device is accepted.
- For reason `compromised`, verifiers SHOULD also distrust messages from that device timestamped up to 24 hours before `at`.

## Verifying an identity

To check that a device acts for a user, a verifier needs the user's **identity bundle**: the log from genesis to head.

1. Check that the genesis hash equals the user ID.
2. Walk the log, checking each `prev` link, signature and signer, and applying the recovery-window rule to any fork.
3. The device is valid if it has an `identity.device` record, no later `identity.revoke`, and no passed `expires`.

The **identity head** is the hash of the latest record. Peers exchange heads during the handshake ([02](02-transport.md)), so a stale cache shows up immediately.

Bundles are small: a few hundred bytes per record. Nodes cache the bundles of their contacts and community members. Bundles can be fetched from:
- the user's own devices, with the `identity.get` request
- the user's mailbox hosts, which MUST serve the bundle of every user they host
- HTTPS, at `https://<handle domain>/.well-known/nekochat/identity/<user-id>`

## Adding a device

1. The new device generates its device key and shows it as a QR code or short link.
2. A device that holds the root key signs an `identity.device` record for it. Instance users do this in the browser: logging in decrypts the root key locally, so the instance still never sees it.
3. The signing device sends the new device the updated bundle and the contact key, over a direct Iroh connection between the two devices.
4. Existing contacts learn about the new device the next time they fetch the bundle or see a new identity head. Their MLS groups then add the device ([04](04-messaging.md)).

## Profile

```cddl
profile-body = {
  "user": user-id,
  "seq": uint,                      ; highest seq wins
  ? "name": tstr .size (1..64),      ; display name
  ? "avatar": blob-ref,             ; 05-blobs.md
  ? "banner": blob-ref,
  ? "bio": tstr .size (0..1000),
  ? "pronouns": tstr .size (0..40),
  ? "handles": [* tstr],            ; claimed handles, see below
}
```

Signed by any valid device of the user, as record type `profile`.

- A profile is only shared with contacts and community members, and with directories if the user opts in ([07](07-discovery-and-federation.md)).
- Strangers see only what the user publishes in their reachability record ([03](03-contacts-and-mailboxes.md)).

```cddl
profile-get-params = { "user": user-id, ? "seq-above": uint }
profile-get-result = { ? "profile": bytes }      ; omitted when nothing newer than seq-above
```

## Handles

A handle is a readable name such as `nyaeko@randomnyachat.com`. It's a pointer to a user ID, never the identity itself. A handle is valid only when both sides agree:

1. The user lists the handle in their profile's `handles`.
2. The domain serves a signed handle binding at
   `https://randomnyachat.com/.well-known/nekochat/handles/nyaeko`.

```cddl
handle-body = {
  "handle": tstr,               ; "nyaeko@randomnyachat.com"
  "user": user-id,
  "issued": timestamp,
}
```

The binding is signed by the instance's server key ([07](07-discovery-and-federation.md)), as record type `handle`.

- If either side drops the handle, it stops resolving.
- Moving to another instance gives the user a new handle but keeps their user ID, contacts and communities.
- A light node with no domain has no handle. People reach it by user ID, a QR code or an invite link.

## Open questions

- **Recovery window length:** whether 72 hours is right, and whether a user can choose it.
- **Revoked devices:** whether `identity.revoke` should also carry a list of message IDs to hide, for a compromised device that sent messages before it was revoked.
