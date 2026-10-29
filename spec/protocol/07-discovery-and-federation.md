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
  ? "sign-in": [+ {                  ; 01-identity.md, Instance sign-in
    "method": "password" / "passkey" / "oidc" / "oauth",
    ? "provider": tstr,              ; "google", "discord", … for oidc and oauth
  }],
  ? "linking": [* tstr],             ; platforms this instance attests linked accounts for
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
| Linked account (inside a user listing) | The attesting instance, or the user for a self-proof | Off by default, per link |
| Area (user or community) | A device of the user, or the community's home node | Off by default. A user's area expires within 24 hours. |

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
  ? "links": [* bytes],             ; signed link or link.claim records the user marked searchable
  "discoverable": bool,
}
```

### Directory API (HTTPS)

| Request | Purpose |
|---|---|
| `GET /v1/info` | The directory's name, operator, privacy policy URL, and which features it supports (`area`, `links`) |
| `POST /v1/announce` | Body: a signed instance, community, user or area listing, or a withdrawal (`application/cbor`) |
| `GET /v1/instances?cursor=&lang=&registration=` | List instances |
| `GET /v1/communities?q=&tags=&lang=&nsfw=&cursor=` | Search communities |
| `GET /v1/users?q=&cursor=` | Search opted-in users |
| `GET /v1/users/by-link?platform=&username=` | Find users by a searchable linked account ([below](#finding-people-by-linked-account)) |
| `GET /v1/area?cell=&kind=` | Find users or communities in an area ([below](#finding-people-and-communities-by-area)) |
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

## Finding people by linked account

Users can let people find them by an account on another platform, such as their Discord username ([01](01-identity.md#linked-accounts)). It's off by default, per link.

- A user's `listing.user` carries the link records they marked `searchable`.
- Directories MUST verify each link before indexing it:
  - an attested `link`: by its signature and the attesting instance's record
  - a `link.claim`: by fetching its proof URL
- `GET /v1/users/by-link` answers **exact matches only**. The platform name is required, and the username or account ID must match completely.
  - No partial matches, prefixes or wildcards.
  - No way to list every user with a link to a given platform.
  - A query can't name more than one account, so nobody can match a whole friend list at once.
- Directories MUST rate-limit by-link lookups per client. They SHOULD allow no more than 30 an hour.
- When a user marks a link unsearchable, removes it, or the attesting instance signs a `link.revoke`, directories MUST drop it within 24 hours.

## Finding people and communities by area

Users and communities can choose to be found by rough location. It's off by default. A user turns it on deliberately, and it switches itself off ([expiry, below](#area-rules)).

### Area listing

Record type `listing.area`:

```cddl
area-listing-body = {
  "subject": user-id / community-id,
  "kind": "user" / "community",
  "seq": uint,
  ? "cell": tstr .size (3..5),      ; geohash, absent when withdrawn
  ? "label": tstr .size (1..64),    ; a place name the user picked, e.g. "Seattle"
  "at": timestamp,
  "expires": timestamp,
  ? "withdrawn": true,
}
```

- Signed by one of the user's devices, or by the community's home node.
- The area is a **geohash cell**. Precision 5 is about 5 km across, precision 4 about 40 km, and precision 3 about 150 km.
- A listing MUST NOT be more precise than 5 characters. Directories MUST reject longer cells.

### Area rules

These protect against the known ways location features get abused:

- **The device rounds, not the directory.** The client turns the device's location into a cell locally, and only the cell is ever sent. Exact coordinates MUST NOT leave the device. The user can also type a place and pick its cell, with no location permission at all.
- **No distances.** A directory answering `GET /v1/area` returns listings in the requested cell and its 8 neighbours.
  - Each result says only `"same-cell"` or `"nearby-cell"`.
  - Results MUST NOT include distances, and MUST NOT be sorted by distance; they're returned in random order.
  - This stops anyone working out a precise position by searching from several spots, the attack that has exposed users of dating apps.
- **Listings expire.**
  - A user's area listing MUST expire within 24 hours of `at`. The client refreshes it only while the user keeps the feature on.
  - A community's MAY last up to 30 days.
  - Directories MUST delete expired listings, and MUST NOT keep any history of past cells.
- **Moving reveals a route.** Clients SHOULD NOT update the cell automatically while the user travels. They SHOULD ask, or update at most once every 6 hours, because a series of cells traces where someone went.
- **Searches are limited.** Directories MUST rate-limit area queries per client, and SHOULD refuse clients that query many distant cells in a short time, since that pattern means someone is sweeping the map.
- **Searching doesn't require sharing.** A directory MUST NOT require the searcher's own location.
- **Blocks can't be enforced by directories.** A directory doesn't know who has blocked whom, so a blocked person may still see someone in area results. They still can't contact them, because contact needs acceptance ([03](03-contacts-and-mailboxes.md)). Clients MUST tell users this before they turn the feature on.
- **Minors:** an instance that knows a user is under 18 MUST NOT publish an area listing for them, and clients SHOULD NOT offer the feature to them.

### Withdrawing

Turning the feature off announces a `listing.area` with `withdrawn: true` and a higher `seq`. Directories MUST delete the area listing within 1 hour. That's stricter than the 24 hours for other listings, because location is more sensitive.

## People nearby on the same network

A second, more private form needs no directory: finding people on the same local network, such as at an event or café. Location never leaves the room.

- Iroh can already find other nodes on the local network.
- When the user turns on **Visible nearby**, their device answers `nearby.get` from peers on the local network:

```cddl
nearby-card = {
  "user": user-id,
  ? "name": tstr,
  ? "avatar-thumb": bstr .size (0..16384),   ; small unencrypted image
  "reach": bytes,                 ; signed reach record, so the finder can send a contact request
  "expires": timestamp,
}
```

  `nearby-card` is signed by one of the user's devices, as record type `nearby.card`.
- A device MUST answer `nearby.get` only for peers whose connection comes from a local network address (private, link-local or the same subnet). It MUST refuse it over relays.
- Visible nearby MUST switch itself off after at most 2 hours, and is off whenever the device joins a network it hasn't been told to trust.
- Finders see a list of cards and can send a contact request. Nothing else is shared until it's accepted.
- Bluetooth discovery for phones can use the same card later (Phase 7 and after).

## Personal data

Linked accounts and areas are personal data under privacy laws such as the GDPR. Each operator of an instance or directory that offers them is legally responsible for that data.

Whatever the local law, the protocol requires that operators:
- keep only the current record: no history of areas, and no links the user has removed
- delete a user's listings, links and areas within the times above when the user withdraws them, and within 30 days of any other deletion request
- publish a privacy policy URL, in the instance record's `rules-url` or the directory's `/v1/info`
- don't sell, share or reuse the data outside discovery

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
- **Area queries and Sybil clients:** rate limits per client are weak against many fake clients. Directories could require a token from the searcher's instance, at the cost of light-node users.
- **Hiding from specific people in area results:** whether a user could give directories a list of hashed user IDs to exclude, and whether that leaks more than it protects.
