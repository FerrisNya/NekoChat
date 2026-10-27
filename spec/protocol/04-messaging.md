# 04. Messaging

Every conversation is an MLS group. That covers a DM, a group DM and an encrypted community channel. Messages travel as envelopes, either directly between devices or through mailboxes. Unencrypted community channels use signed plaintext instead, described at the end.

## MLS profile

- **Ciphersuite:** `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519` (0x0001). Its signature scheme is Ed25519, so the MLS leaf signature key *is* the device key.
- **Credential:** `BasicCredential`. Its `identity` field holds the deterministic CBOR of `{ "user": user-id, "device": device-id }`.
- **Credential validation:** a member is valid only if its leaf's signature key equals `device`, and `device` is currently certified for `user` ([01](01-identity.md)). Every member checks this on every Add and Update.
- **Wire format:**
  - Application messages are `PrivateMessage`, so even the sender's identity is encrypted.
  - Commits and proposals are `PublicMessage` in community channels, so the home node can check them ([06](06-communities.md)). In DMs and group DMs they're `PrivateMessage`.
- **Every device is a leaf.** A user with three devices has three leaves in each of their groups.
- **KeyPackages:**
  - Each device keeps at least 10 unused KeyPackages available through `keypackage.get` (contacts only).
  - It puts one in each contact request.
  - KeyPackages expire after 30 days.

```cddl
keypackage-get-params = { "user": user-id }
keypackage-get-result = { "packages": [* { "device": device-id, "kp": bytes }] }   ; one per device
```

## Envelope

The unit that moves between nodes. The envelope is plaintext; its payload isn't.

```cddl
envelope = {
  "v": 1,
  "kind": "mls" / "plain" / "contact-request",
  ? "group": bstr .size 16,          ; MLS group ID (mls) or channel key (plain)
  "payload": bstr,                   ; MLSMessage (mls), signed-content (plain)
  ? "ephemeral": true,               ; do not store in mailboxes
}
```

The `contact-request` kind is defined in [03](03-contacts-and-mailboxes.md).

Delivery, in order of preference:
1. **Direct:** `deliver` to each recipient device the sender can reach.
2. **Mailbox:** `mailbox.deposit` to the recipient user's mailbox hosts, for devices that weren't reachable directly.
3. **Community:** through the home node, for community channels ([06](06-communities.md)).

```cddl
deliver-params = { "envelope": bytes }
deliver-result = { "accepted": bool }
```

`deliver` is only accepted from contacts or fellow group members. A device MUST NOT assume delivery happened unless it got `accepted: true` or the deposit succeeded. Until then the envelope stays in the sender's outbox.

Envelopes marked `ephemeral` are delivered directly or dropped. Mailboxes MUST refuse them. That covers typing indicators and presence.

## Groups

| Group | Members | Who may commit |
|---|---|---|
| DM | 2 users, all their devices | Either user's devices |
| Group DM | Up to 25 users | Any member device |
| Community channel | Channel readers, per the community's permissions | Members, with the home node as orderer and external sender ([06](06-communities.md)) |

- **Group ID:** 16 random bytes, chosen by the creator. For a community channel, it's recorded in the channel's metadata.
- **New device on a contact's account:** when a member sees a new device in a contact's identity bundle, it fetches a KeyPackage for it and commits an Add. **Revoked device:** the member commits a Remove. Any member may do either.

### Commit conflicts without an orderer

DMs and group DMs have no home node, so two devices can commit in the same epoch. The rule:

1. A device that receives two valid commits for the same epoch applies the one with the **lowest hash** of its MLS content (`MLSContent` in RFC 9420).
2. The device whose commit lost discards its pending state. If its change still matters, it proposes it again in the next epoch.
3. Application messages sent in the losing epoch are re-encrypted and re-sent by their senders, who get the winning commit and see that their epoch is gone.

Devices keep the previous epoch's secrets for 5 minutes, so messages crossing a commit in flight still decrypt.

## Message content

Inside an MLS application message, or inside signed content for plain channels:

```cddl
content = {
  "v": 1,
  "id": neko-id,                       ; unique message ID
  "type": content-type,
  "at": timestamp,
  ? "channel": neko-id,                ; community channels only
  ? "body": body,
  ? "attachments": [* attachment-ref], ; 05-blobs.md, at most 10
  ? "reply": neko-id,
  ? "mentions": [* user-id],
  ? "mention-roles": [* neko-id],
  ? "target": neko-id,                 ; for edit, delete, reaction, pin
  ? "emoji": tstr,                     ; for reaction
  ? "card": contact-card,              ; for the first DM message, 03
  ? "nonce": tstr .size (0..64),       ; client-side de-duplication, as Fluxer uses today
}

content-type = "post" / "edit" / "delete" / "reaction" / "unreaction" /
               "pin" / "unpin" / "read" / "typing" / "card" / "system"

body = {
  "text": tstr,                       ; ≤ 16 000 bytes
  "format": "markdown",               ; Fluxer's markdown dialect
  ? "embeds": [* embed],              ; link previews built by the sender (05-blobs.md)
}
```

### Rules

- **Who sent it:**
  - In MLS groups, the author is the user whose device sent the MLS message. Receivers take it from the MLS sender leaf, never from the content.
  - In plain channels, the author is the signer ([below](#unencrypted-community-channels)).
- **edit:** only the original author may edit. The edit replaces `body`, `attachments` and `mentions`, and it carries the new values with `target` set to the edited message's ID. Receivers keep the original's ID and position and show it as edited.
- **delete:**
  - The author may delete their own message.
  - In a community, a member with the "manage messages" permission may delete anyone's message.
  - Receivers MUST remove the body and attachments from storage and show a placeholder, or nothing.
  - Mailboxes and home nodes MUST drop any stored copy they can identify.
- **reaction / unreaction:** one per user per emoji per message.
- **read:** "I've read up to this message". Sent only to the user's own devices, through the device-sync group (below), never to other users, unless a later setting allows read receipts.
- **typing:** always `ephemeral`.

### Ordering

| Where | Order |
|---|---|
| Community channels | By the home node's sequence number ([06](06-communities.md)). Final and the same for everyone. |
| DMs and group DMs | By the `neko-id` time prefix, then by the full `neko-id`. Messages arriving late are inserted in place. |

### Message IDs in the web app

The web app assumes 64-bit snowflake IDs and reads timestamps from them in about 52 files ([cut-lines §11](../cut-lines.md)). A neko-id keeps the timestamp in its first 48 bits, so:
- **Timestamp parsing:** the helpers change in one place.
- **Sorting and string handling** need updating wherever IDs are compared as 64-bit integers.

This is client work for Phase 2, not a protocol issue.

## Your own devices

Each user has a **device-sync group**: an MLS group containing only their own devices. It carries:
- read positions
- settings changes
- drafts, if enabled
- copies of messages the user sent from another device, when that device isn't in a group yet

`device.sync` (own devices only) lets a newly added device fetch history from an existing one:

```cddl
device-sync-params = { "since": timestamp, ? "groups": [* bstr] }
; streams { "group": bstr, "content": bytes } frames, re-encrypted for the new device inside the sync group
```

## Presence

`presence.subscribe` (contacts only) streams presence frames:

```cddl
presence = { "status": "online" / "idle" / "dnd" / "invisible-offline", ? "activity": tstr, "at": timestamp }
```

Presence is never stored or forwarded through mailboxes. A user who appears invisible sends `invisible-offline`, or simply refuses the subscription.

## Unencrypted community channels

A community can turn encryption off per channel ([06](06-communities.md)). Messages in such a channel are signed instead.

```cddl
signed-content = {
  "content": bytes,                  ; deterministic CBOR of content
  "device-cert": bytes,              ; the author's identity.device record
  "sig": sig,                        ; by the device key, over "NekoChat-v1:content" ‖ 0x00 ‖ content
}
```

Envelope `kind: "plain"`, with `group` set to the channel's 16-byte key.

- The home node can read these, store them, index them for search and show them to anyone with read permission, including people who join later.
- That gives plain channels full history, at the cost of the host being able to read them.

## Open questions

- **History for new members of encrypted channels:** options include an encrypted history key per channel handed to new members on join, or older messages re-shared by members. Without one, new members see only messages sent after they join.
- **Commit conflict rule:** the lowest-hash rule needs testing with flaky connections. A DM could also nominate one side's home instance as orderer when both have one.
- **Read receipts:** whether to support them between users, and how they interact with privacy.
