# 07. Discovery and federation

Discovery finds every instance, plus any peer-to-peer node or community that chose to be listed. It honours two kinds of "no":
- **Opt-out:** something that doesn't want to be listed is never listed.
- **Defederation:** an instance that blocks another doesn't show it, reach it or accept anything from it.

Discovery only shows public information. Contacting a user found through it still needs a contact request and their acceptance ([03](03-contacts-and-mailboxes.md)).

## Instances

### Server key

Every instance has an Ed25519 **server key**. It's the instance's Iroh node ID, and it signs:
- the instance record
- handle bindings ([01](01-identity.md))
- home-node log entries, when the instance hosts communities ([06](06-communities.md))

### Instance record

Record type `instance`, signed by the server key:

```cddl
instance-body = {
  "domain": tstr,                    ; "randomnyachat.com"
  "server": pubkey,
  "seq": uint,
  "dial": bytes,                     ; Iroh NodeAddr
  "name": tstr .size (1..100),
  ? "description": tstr .size (0..2000),
  ? "icon": blob-ref,
  "software": { "name": tstr, "version": tstr },
  "protocols": [+ uint],             ; supported protocol versions, e.g. [1]
  "registration": "open" / "approval" / "invite" / "closed",
  "federation": "open" / "allowlist",
  "discoverable": bool,              ; false = do not list this instance
  ? "languages": [* tstr],           ; BCP 47 tags
  ? "rules-url": tstr,
  ? "contact": tstr,
  ? "services": {                    ; what the instance offers non-members
    ? "relay": tstr,                 ; public relay URL
    ? "mailbox": bool,               ; hosts mailboxes for non-members
  },
  "at": timestamp,
}
```

- Served at `https://<domain>/.well-known/nekochat/instance` as `application/cbor`.
- Served over Iroh with `instance.info`.
- The domain proves itself through HTTPS: a record is valid only if the copy at that domain's well-known URL names the same server key.

## Federation

Instances talk to each other over `nekochat/1`, handshaking with `role: "instance"` ([02](02-transport.md)). Federation traffic is the same traffic as everything else:
- deposits into mailboxes the instance hosts
- deliveries to accounts' device keys
- home-node requests for communities it hosts

There is no separate federation protocol to learn.

### Defederation

Record type `instance.blocks`, signed by the server key:

```cddl
blocks-body = {
  "server": pubkey,
  "seq": uint,
  "blocked": [* {
    ? "domain": tstr,
    ? "server": pubkey,
    "level": "suspend" / "silence",
    ? "reason": tstr,
  }],
  "public": bool,                    ; whether this list is published
}
```

| Level | Effect on the blocking instance |
|---|---|
| `suspend` | Refuses connections from the blocked instance's server key and its accounts' device keys. Refuses deposits from them. Hides its communities and accounts from its users' discovery. |
| `silence` | Hides the blocked instance from discovery. Existing contacts and joined communities keep working. |

- With `federation: "allowlist"`, an instance refuses every instance not on its own allowlist. The allowlist stays local.
- Individual users can block whole instances too. Their devices apply it locally, the same way.
- If `public` is true, the list is served at `/.well-known/nekochat/blocks`.

Defederation can't reach into end-to-end encrypted conversations between users who are already contacts. It can stop the blocking instance from carrying their traffic. Users on light nodes or other instances are unaffected, because they don't use that instance.

## Directories

A **directory** is a service that collects public listings and answers searches. Anyone can run one. Clients come with a default directory list, which each instance can change for its users.

### What can be listed

| Listing | Signed by | Opt-in |
|---|---|---|
| Instance | Server key (the instance record) | `discoverable: true` |
| Community | Its home node | The community's `discoverable` setting (`op.settings`) |
| User | One of the user's devices | Off by default. The user turns it on. |

Record type `listing.community`, signed by the home node:

```cddl
community-listing-body = {
  "community": community-id,
  "seq": uint,
  "home": pubkey,
  ? "instance": tstr,
  "name": tstr,
  ? "description": tstr .size (0..2000),
  ? "icon": blob-ref,               ; unencrypted blob (05-blobs.md)
  ? "banner": blob-ref,
  ? "tags": [* tstr],
  ? "languages": [* tstr],
  "members": uint,                  ; approximate
  "nsfw": bool,
  ? "invite": tstr,                 ; a permanent public invite code
  "discoverable": bool,
}
```

Record type `listing.user`, signed by a device of the user:

```cddl
user-listing-body = {
  "user": user-id,
  "seq": uint,
  ? "name": tstr,
  ? "avatar": blob-ref,             ; unencrypted
  ? "bio": tstr .size (0..500),
  ? "handle": tstr,
  "discoverable": bool,
}
```

### Directory API (HTTPS)

| Request | Purpose |
|---|---|
| `POST /v1/announce` | Body: a signed instance, community or user listing (`application/cbor`) |
| `GET /v1/instances?cursor=&lang=&registration=` | List instances |
| `GET /v1/communities?q=&tags=&lang=&nsfw=&cursor=` | Search communities |
| `GET /v1/users?q=&cursor=` | Search opted-in users |
| `GET /v1/record/<hash>` | Fetch a record as originally signed, so clients can verify it |

Directories MUST:
- verify every listing's signature
- verify that a community listing is signed by the home in its latest `community.home`, by fetching the genesis and delegation from that home
- re-check instance records against the domain's well-known URL at least daily, and drop those that no longer match
- return records as originally signed, so clients can verify them and not trust the directory

Directories SHOULD let clients pass their home instance's block list, or apply it themselves. Clients MUST filter results against their own instance's and their own block lists in any case.

### Gossip between instances

Directories don't have to know everything up front. Instances share the instances they know with `discovery.peers`:

```cddl
peers-params = { ? "after": timestamp }
peers-result = { "instances": [* { "domain": tstr, "server": pubkey, "seen": timestamp }] }
```

- An instance or directory that learns of a new domain fetches and verifies its record before listing it.
- Instances SHOULD NOT include instances they `suspend` in their answers.

### Peer-to-peer nodes

A light node or desktop app with no instance can still be listed. It sends a `listing.user`, or the `listing.community` for a community it hosts, to one or more directories with `POST /v1/announce`.

Iroh publishes each node's dial info through pkarr and the Mainline DHT, so a node can be found by its key without a directory. It can't be *searched for* that way, and it isn't listed unless it announces. Nothing is announced unless the user turns discovery on.

## Opting out

- **To stop being listed,** announce the same listing with `discoverable: false` and a higher `seq`.
  - Directories MUST delete the listing within 24 hours.
  - Directories MUST keep the opt-out record, so gossip or a stale copy can't re-add the entry.
  - Directories MUST serve opt-out records to other directories that ask.
- **Instances** with `discoverable: false` are dropped the same way. Gossip MUST NOT spread them.
- An opted-out community or user keeps working normally. It just can't be found by searching.

## Open questions

- **Directory spam:** how directories should rate-limit announces and resist fake listings. Options include requiring an instance to vouch for each P2P node's listing, or a small proof-of-work.
- **Federated search:** whether directories should query each other, or each crawl independently through gossip.
- **Shared block lists:** whether instances can subscribe to another instance's public block list.
