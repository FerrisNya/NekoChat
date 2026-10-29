# 01. Identity

A user is a chain of signed records that starts with a genesis record. Devices act for the user with their own keys, certified by the user's root key. Nothing here depends on any server: a light node can create and verify identities offline.

## Keys

| Key | Type | Held by | Used for |
|---|---|---|---|
| **Root key** | Ed25519 | One "primary" device, or the home instance in encrypted form ([Instance sign-in](#instance-sign-in)), or offline | Certifying and revoking devices, rotating itself |
| **Recovery key** | Ed25519, optional | Offline: a recovery phrase or paper backup | Replacing a lost or stolen root key |
| **Device key** | Ed25519 | One device, never copied | Iroh node ID, signing messages and records, MLS leaf signature key |
| **Contact key** | X25519 | Every device of the user (shared) | Receiving sealed contact requests ([03](03-contacts-and-mailboxes.md)) |

- A device key MUST NOT be reused across users.
- The root key SHOULD NOT live on every device. A device only needs the root key while it certifies a new device or revokes one.
- Instance accounts: the instance holds a device key for the account's browser sessions. It MAY also hold the root key in encrypted form, so logging in can unlock it ([Instance sign-in](#instance-sign-in)).
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
  ? "links": [* shown-link],        ; linked accounts the user chooses to show, see below
}

shown-link = {
  "record": bytes,                  ; a signed link or link.claim record
  "visibility": "contacts" / "public",
  "searchable": bool,               ; directories may index it (07-discovery-and-federation.md)
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

## Instance sign-in

Signing in only proves who you are to your instance, so it gives your browser access to your account's device key. Your NekoChat identity is still your keys. Light nodes and the desktop app don't sign in to anything.

### Sign-in methods

An instance MAY offer any of:

| Method | Notes |
|---|---|
| Password | Hashed by the instance with Argon2id |
| Passkey | WebAuthn |
| External provider | OpenID Connect or OAuth 2.0: Google, Discord, Facebook, GitHub, Apple, or any OpenID Connect provider the operator configures |

- The instance lists the methods it offers in its instance record ([07](07-discovery-and-federation.md)).
- An account MAY have several methods, and SHOULD keep at least two so losing one provider account doesn't lock the user out.
- An external provider learns only that the user signed in to that instance. It never sees messages, contacts or keys.
- The instance operator registers an app with each provider and follows that provider's terms.
- From an external provider, the instance MUST keep only the provider's stable account ID (`sub` in OpenID Connect). It MUST NOT store the user's email address or contacts from the provider unless the user separately agrees.

### Protecting the root key

The instance MAY store the user's root key so that signing in can unlock it. It stores the key **wrapped**: encrypted once for each unlock method the user has set up. Any one wrap unlocks it.

```cddl
root-wrap = {
  "method": "passkey" / "password" / "recovery" / "custodial",
  ? "credential": bstr,             ; passkey credential ID
  ? "salt": bstr .size 32,          ; password and recovery methods
  ? "kdf": { "m": uint, "t": uint, "p": uint },   ; Argon2id parameters
  "nonce": bstr .size 12,
  "ct": bstr,                       ; ChaCha20-Poly1305 of the 32-byte root private key
}
```

The AEAD's additional data is `"NekoChat-v1 root-wrap" ‖ user-id ‖ method`.

| Method | Where the wrapping key comes from | Can the instance unwrap it? |
|---|---|---|
| `passkey` | HKDF-SHA256 of the passkey's WebAuthn `prf` extension output | No |
| `password` | Argon2id of the user's password (minimum m = 64 MiB, t = 3, p = 1) | No |
| `recovery` | Argon2id of a 24-word recovery phrase (256 bits of entropy) | No |
| `custodial` | A key the instance holds | **Yes** |

- Unwrapping happens in the user's browser. For every method except `custodial`, the instance never sees the plain root key.
- An account that signs in only with an external provider has no password to derive a key from. Its sign-up MUST set up a `passkey` or `recovery` wrap. If the user declines both, it may use `custodial` instead, after being told clearly that the instance could then act as them.
- `custodial` SHOULD be shown in the user's own device list as a warning, so the user can upgrade later.
- When the root key rotates, the instance MUST discard the old wraps and the browser stores new ones.

## Linked accounts

A linked account says "this NekoChat user also controls this account on another platform", such as a Discord or GitHub account. It's only for showing and, if the user allows, for being found. Linking never gives NekoChat access to the other platform's messages or friends.

There are two ways to prove a link.

### Attested by an instance

The user signs in to the platform once through their instance, using the same OAuth or OpenID Connect flow as sign-in. The instance then signs a `link` record with its server key:

```cddl
link-body = {
  "user": user-id,
  "platform": tstr,                 ; "discord", "google", "github", "facebook", "mastodon", …
  "account": tstr,                  ; the platform's stable account ID, never an email address
  ? "username": tstr,               ; display handle at verification time, e.g. "nyaeko"
  "verified": timestamp,
  "attester": tstr,                 ; the attesting instance's domain
}
```

### Claimed with a public proof

For users with no instance, such as light nodes:

1. The user signs a `link.claim` record with one of their devices.
2. They post the claim's record hash on the platform, in a profile field, a public post or a gist, in the form `nekochat-link:<base32 hash>`.
3. Anyone can check the link by fetching `proof-url` and finding that string.

```cddl
link-claim-body = {
  "user": user-id,
  "platform": tstr,
  "account": tstr,
  ? "username": tstr,
  "proof-url": tstr,
  "at": timestamp,
}
```

An instance or directory that checks a claim MAY issue its own `link` attestation for it.

### Showing and trusting links

- Users choose which links appear in their profile, and for each one whether contacts or everyone can see it, and whether it's searchable ([Profile](#profile)).
- Clients show who vouched for each link: the attesting instance's domain, or "self-proof, checked on <date>".
- Clients MUST show a link as **unverified** if its attester is an instance the viewer's instance or the viewer has blocked ([07](07-discovery-and-federation.md)), or if a claim's proof can no longer be found.
- Clients SHOULD mark links verified more than a year ago as **stale**. The user can ask their instance to verify them again.
- **Unlinking:** the user removes the link from their profile. The instance also signs a `link.revoke` record (`{ "user", "platform", "account", "at" }`). Directories that see it MUST drop the link within 24 hours.

### Not supported: importing friend lists

NekoChat doesn't import contact or friend lists from other platforms to find who's already here:
- The major platforms' developer terms largely forbid it.
- Matching uploaded lists reveals who knows whom, and hashing the identifiers doesn't hide them, because short identifiers are easy to guess back from their hashes.

People find each other by looking up one specific username, and only users who marked that link `searchable` can be found that way ([07](07-discovery-and-federation.md)).

## Open questions

- **Recovery window length:** whether 72 hours is right, and whether a user can choose it.
- **Revoked devices:** whether `identity.revoke` should also carry a list of message IDs to hide, for a compromised device that sent messages before it was revoked.
- **Passkey support:** the WebAuthn `prf` extension isn't available in every browser and password manager yet. Users without it fall back to a recovery phrase, which is less convenient.
- **Which platforms to support first,** and whether to support platforms that don't offer OpenID Connect only through public proofs.
