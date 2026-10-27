# 03. Contacts and mailboxes

Nothing is shared with another user until they accept. Before acceptance, a stranger can do exactly one thing: drop a sealed contact request into your mailbox. Your devices' addresses, your profile, your presence and every message stay private until you say yes.

## Reachability record

What a stranger is allowed to know about you. Record type `reach`:

```cddl
reach-body = {
  "user": user-id,
  "seq": uint,                        ; highest seq wins
  "mailboxes": [+ mailbox-ref],       ; where to deposit requests
  "contact-key": bstr .size 32,       ; X25519 public key (01-identity.md)
  "accepting": bool,                  ; false = not taking contact requests
  ? "public-name": tstr .size (1..64),  ; optional name shown on requests
  ? "requires-note": bool,            ; reject requests with no note
}

mailbox-ref = {
  "node": pubkey,                    ; the mailbox host's node ID
  ? "relay": tstr,                   ; relay URL, as a dialling hint
  ? "https": tstr,                   ; HTTPS deposit URL, for browsers
}
```

Signed by any valid device of the user.

- The record lists mailbox hosts, never the user's own devices.
- Anyone may fetch it with `reach.get` from a mailbox host, or over HTTPS from the user's handle domain: `/.well-known/nekochat/reach/<user-id>`.

```cddl
reach-get-params = { "user": user-id }
reach-get-result = { "reach": bytes, "identity": [* bytes] }   ; signed reach record + identity bundle
```

## Mailboxes

A mailbox host stores envelopes for a user until the user's devices collect them. Any node can host mailboxes:
- an instance, for its members
- a friend's always-on node
- the user's own always-on node

A user SHOULD list at least two mailbox hosts, so one being down doesn't block delivery.

### What a host may accept

A host MUST store a deposit only if one of these is true:
1. It's a **contact request**: `kind: "contact-request"`, within the rate limits below.
2. It carries a valid **delivery token** issued by the recipient.
3. It comes from one of the recipient's own devices.

Everything else MUST be refused with `unauthorized`. This is what enforces consent: a stranger can't fill your mailbox with messages, because they have no token.

### Delivery tokens

A recipient issues a token to each sender it accepts. Record type `mailbox.token`:

```cddl
token-body = {
  "recipient": user-id,
  "sender": user-id / pubkey,      ; a user, or a community home node's key
  "scope": "contact" / "contact-reply" / tstr,   ; tstr form: "community:" + base32 community-id
  "issued": timestamp,
  ? "expires": timestamp,
  "nonce": bstr .size 16,
}
```

Signed by one of the recipient's devices.

- The host checks the signature against the recipient's identity bundle, and that the depositing connection authenticated as `sender` (a device of that user, or that home node's key).
- **Revocation:** the recipient sends its hosts a revocation list. Hosts MUST refuse tokens on it.

```cddl
revocations-body = {
  "user": user-id,
  "seq": uint,
  "revoked": [* hash],             ; record hashes of revoked tokens
}
```

Record type `mailbox.revocations`, signed by a device of the user, sent with `mailbox.revocations.put`.

### Mailbox methods

```cddl
deposit-params = {
  "to": user-id,
  "envelope": bytes,               ; an envelope (04-messaging.md)
  ? "token": bytes,                ; signed mailbox.token
}
deposit-result = { "id": neko-id }

fetch-params = { ? "after": neko-id, ? "limit": uint }        ; own devices only
fetch-result = { "items": [* { "id": neko-id, "envelope": bytes, "received": timestamp }] }

ack-params = { "ids": [+ neko-id] }                           ; own devices only
```

`mailbox.subscribe` (own devices only) streams `{ "id", "envelope", "received" }` frames as deposits arrive, so an online device doesn't need to poll.

- A host deletes an item once every device that was valid when it arrived has acknowledged it, or after 30 days.
- Hosts SHOULD encrypt stored items at rest.
- Envelopes are already end-to-end encrypted, so a host learns only the recipient, the depositing node, the size and the time.

## Contact requests

### Sending

When user A wants to reach user B:

1. A gets B's user ID: from a QR code, a link, a handle lookup or a directory listing.
2. A fetches B's reachability record and identity bundle.
3. A builds a contact request and **seals** it to B's contact key with HPKE (RFC 9180, base mode). The suite is DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20-Poly1305, with info string `"NekoChat-v1 contact-request"`.
4. A deposits the sealed request at one of B's mailbox hosts.

Record type `contact.request`, signed by one of A's devices:

```cddl
contact-request-body = {
  "from": user-id,
  "to": user-id,
  "id": neko-id,
  "identity": [* bytes],            ; A's identity bundle
  "reach": bytes,                   ; A's signed reach record
  "reply-token": bytes,             ; mailbox.token from A, scope "contact-reply", so B can answer
  "key-packages": [+ bytes],        ; MLS KeyPackages, one per A device (04-messaging.md)
  ? "note": tstr .size (0..500),
  ? "via": user-id,                 ; a mutual contact, if A found B through them
  "at": timestamp,
}
```

The envelope carrying it:

```cddl
contact-request-envelope = {
  "v": 1,
  "kind": "contact-request",
  "enc": bstr,                      ; HPKE encapsulated key
  "ct": bstr,                       ; HPKE ciphertext of the signed request
}
```

The mailbox host sees only that *someone* sent B a request. It can't read who sent it or what it says, though it does see which node deposited it.

### Rate limits

A mailbox host MUST enforce, per recipient:
- at most 1 pending request per depositing node per 24 hours
- at most 50 requests per 24 hours in total, after which further requests are refused with `rate_limited`

B's own devices MUST also drop:
- repeated requests from a user they have already rejected or blocked
- requests with no note when the reach record says `requires-note: true`

### Answering

B's devices show the request: A's public name, the note, A's handle if A's identity has a valid one, and whether a mutual contact is named in `via`.

**Accept.** B:
1. issues A a delivery token (scope `contact`)
2. creates the DM's MLS group, adding all of A's devices from A's KeyPackages ([04](04-messaging.md))
3. sends A a `contact.accept`, sealed to A's contact key and deposited in A's mailbox with A's reply token

Record type `contact.accept`, signed by one of B's devices:

```cddl
contact-accept-body = {
  "from": user-id,                  ; B
  "to": user-id,                    ; A
  "request": neko-id,
  "token": bytes,                   ; mailbox.token for A to deposit into B's mailboxes
  "card": contact-card,
  "welcome": bytes,                 ; MLS Welcome for the DM group
  "at": timestamp,
}

contact-card = {
  "identity": [* bytes],            ; B's identity bundle
  "profile": bytes,                 ; B's signed profile
  "devices": [* {
    "device": device-id,
    "dial": bytes,                  ; Iroh NodeAddr, so A can connect directly
  }],
}
```

A answers with its own contact card in the DM group's first message. From then on, the two devices connect directly when they can, and fall back to each other's mailboxes when they can't.

**Reject.** B drops the request. Nothing is sent, so A can't tell a rejection from a request that hasn't been read yet.

**Ignore.** The request expires after 30 days, along with the reply token.

## Blocking

To block A, B's devices:
1. add A's token to the revocation list and push it to every mailbox host
2. leave the DM's MLS group, or remove A from it
3. stop accepting direct connections from A's devices
4. drop A's future contact requests locally

B MAY also give its mailbox hosts a list of blocked user IDs, so the host drops their contact requests too. This is off by default, because it shows the host who B has blocked.

Blocking sends A nothing.

## Removing a contact

Same as blocking, but A's future contact requests are allowed again.

## Open questions

- **Sealed sender:** hiding the depositing node from mailbox hosts, perhaps by depositing through a relay with an anonymous token.
- **Spam:** whether rate limits are enough, or whether to add proof-of-work or require an introduction from a mutual contact on busy accounts.
- **Contact key replacement:** when a revoked device had the shared contact key, existing contacts need the new one. The draft relies on them fetching the updated reach record.
