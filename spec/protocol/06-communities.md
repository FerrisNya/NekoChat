# 06. Communities

A community (a "guild" in Fluxer's code) has one **home node** that orders everything that happens in it: changes to channels, roles, members, and the messages in each channel. Members keep replicas, so they can read and verify history while the home node is offline. Only new changes have to wait for it.

The home node can be an instance or a member's own node. Either way, it gets no special trust beyond ordering:
- It can't forge another member's changes, because every change is signed by its author.
- It can't rewrite history without members noticing, because the log is hash-chained and members keep copies.
- In encrypted channels it can't read messages.

## Community ID and genesis

Record type `community.genesis`, signed by the owner's device:

```cddl
community-genesis-body = {
  "owner": user-id,
  "name": tstr .size (1..100),
  "created": timestamp,
  "nonce": bstr .size 16,
}
```

```
community-id = BLAKE3(record bytes of the genesis record)
```

## Home delegation

The owner names the home node. Record type `community.home`, signed by the owner's device:

```cddl
home-body = {
  "community": community-id,
  "seq": uint,                      ; highest seq wins
  "home": pubkey,                   ; the home node's key (instance server key or device key)
  ? "instance": tstr,               ; domain, if the home is an instance
  "from": timestamp,
}
```

Members MUST accept ordering only from the node named in the highest-seq valid `community.home`.

## The community log

Every change to a community is an **operation**, signed by the member who made it. The home node checks each operation against the permissions in force at that point. If the operation is allowed, the home appends it to the log.

```cddl
log-entry-body = {
  "community": community-id,
  "seq": uint,                      ; starts at 1, no gaps
  "prev": hash,                     ; hash of entry seq-1; for seq 1, the genesis hash
  "op": bytes,                      ; the signed operation, as received
  "at": timestamp,
}
```

Record type `community.entry`, signed by the home node.

Operations are signed records whose type starts with `op.`. Every body includes `"community": community-id` and `"id": neko-id`.

| Operation | Body (beyond community and id) | Who may author it |
|---|---|---|
| `op.settings` | name, icon blob-ref, description, default permissions, discoverable, verification level | "Manage community" |
| `op.channel.create` / `update` / `delete` | channel ID, name, kind (text, voice, category, …), parent, position, topic, `encrypted: bool`, permission overwrites, MLS group ID | "Manage channels" |
| `op.role.create` / `update` / `delete` | role ID, name, colour, permissions, position, hoist, mentionable | "Manage roles", only below the author's top role |
| `op.member.roles` | user ID, role IDs | "Manage roles", only below the author's top role |
| `op.member.join` | user ID, invite code | The joining user |
| `op.member.leave` | — | The leaving user |
| `op.member.kick` / `ban` / `unban` | user ID, reason, ban expiry | "Kick" or "ban members" |
| `op.member.timeout` | user ID, until | "Moderate members" |
| `op.invite.create` / `revoke` | code, expires, max uses | "Create invites" |
| `op.owner.transfer` | new owner user ID | The owner. The new owner then signs a new `community.home` to keep or change the home. |

Permission names and bits come from Fluxer's existing model and are listed when Phase 5 ports it ([roadmap](../roadmap.md)). The semantics are Fluxer's: role hierarchy, channel overwrites, the owner bypassing everything.

### Checking the log

A member that receives log entries:
1. checks each entry's signature against the current home key, its `seq` and its `prev`
2. checks the operation's author signature and device certificate
3. replays the permission check itself

An entry that fails is **disputed**. The member stops applying the log, keeps what it has and warns the user. A misbehaving home can't silently grant someone powers they don't have.

## Channel messages

Messages are not log entries. Each channel has its own sequence, so a busy channel doesn't block community changes.

1. A member sends the envelope to the home with `community.post`.
2. The home checks the sender may post in the channel. It identifies the sender from the authenticated connection, plus the MLS sender in a PublicMessage, or the device certificate in plain channels.
3. The home assigns the next channel sequence number and returns a receipt.
4. The home pushes the envelope and receipt to subscribed members, and deposits them in offline members' mailboxes using each member's community delivery token.

```cddl
post-params = { "community": community-id, "channel": neko-id, "envelope": bytes }

receipt-body = {
  "community": community-id,
  "channel": neko-id,
  "seq": uint,
  "envelope-hash": hash,
  "prev": hash,                    ; previous receipt's hash in this channel
  "at": timestamp,
}
```

Record type `community.receipt`, signed by the home.

- The home can't read encrypted messages, so content rules (like "delete" being allowed only for the author or moderators) are enforced by receivers. They check against the community log when they decrypt.
- For plain channels, the home MAY enforce them itself.

### Requests to the home node

| Method | Purpose |
|---|---|
| `community.post` | Submit a channel message |
| `community.op` | Submit a signed operation for the log |
| `community.sync` | `{ community, ? log-after: seq, ? channels: [{ channel, after: seq }] }`: fetch log entries and channel messages with receipts |
| `community.subscribe` | Stream new log entries, messages and receipts for the community |
| `community.join` | `{ invite code, identity bundle, delivery token (scope community), key packages }` |
| `community.groupinfo` | Fetch the latest MLS GroupInfo for an encrypted channel the caller may read |
| `community.blob.pin` | Ask the home to pin an attachment ([05](05-blobs.md)) |

Home nodes MUST serve `community.sync` only to members, or to anyone for channels readable by `@everyone` in a discoverable community.

## Joining

1. The joiner gets an invite. It's a link carrying the community ID, the home node's dial info and an invite code.
2. The joiner fetches the community's public listing, if it has one ([07](07-discovery-and-federation.md)), and shows it.
3. The joiner calls `community.join`. The home checks the invite, appends an `op.member.join` and stores the delivery token, so it can reach the member's mailboxes.
4. The joiner syncs the log and the channels it may read.

Invite link format (shown as text; this is a display format, not wire):

```
https://<instance>/invite/<code>           (instance-hosted)
nekochat://invite/<community-id>/<code>?home=<node-id>&relay=<url>   (any)
```

## Encrypted channels

Each encrypted channel is one MLS group. The home node is **not** a member, so it never gets the keys.

- **Joining** uses an MLS **external commit**:
  1. The joiner fetches the channel's GroupInfo with `community.groupinfo`.
  2. It adds itself with an external commit.
  3. The home accepts that commit only if the log says the joiner may read the channel, and orders it like any other message.

  Members SHOULD keep the home's copy of GroupInfo current by sending it a fresh one after each commit.
- **Removal** (kick, ban, losing a role or channel access): the home is configured as an MLS **external sender** (the `external_senders` group extension). It sends a Remove proposal, and the next member to send anything in the channel MUST commit it first.
  - Until that commit, the removed member can still decrypt new messages.
  - Clients SHOULD commit pending removals immediately when they see them, not wait for their next message.
- **Permission changes** that add readers are handled the same way: the newly allowed members join by external commit.
- **History:** new members can't read messages from before they joined. See [04](04-messaging.md#open-questions).

### Unencrypted channels

Channels with `encrypted: false` use signed plaintext ([04](04-messaging.md#unencrypted-community-channels)). The home stores and serves their full history and can index it for search. Large public communities will often want this.

## When the home is offline

- Members keep reading everything they've synced.
- New messages and operations queue on the author's device and show as pending.
- They're submitted when the home is back. The home orders them as they arrive, so a queued message may land after others sent later.
- Voice channels on a member-hosted community stop working. Instance-hosted voice uses LiveKit ([roadmap Phase 7](../roadmap.md)).

## Moving the home

1. The owner signs a new `community.home` naming the new node.
2. The new home imports the log and channel history (plain channels' messages, encrypted channels' ciphertexts and receipts) from the old home, or from any member's replica if the old home is gone.
3. It continues the sequences from the last entry it can verify.
4. Members switch to the new home as soon as they see the higher-seq delegation.

If the old home kept appending after the handover, members accept entries from it only up to the delegation's `from` time.

## Open questions

- **Standby home:** whether the owner can sign a standby delegation that takes over automatically when the home has been unreachable for a set time, and how to stop a split if both think they're in charge.
- **Removal lag:** a removed member can read new encrypted messages until someone commits the removal. One option is to let any online member's client commit it automatically within seconds.
- **Very large encrypted channels:** whether to cap encrypted channels at a few thousand readers and recommend unencrypted channels above that.
- **Permission list:** the exact permission bits, fixed in Phase 5.
