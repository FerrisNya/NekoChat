# 05. Blobs

Files, images, avatars and video are **blobs**: content-addressed by BLAKE3 and transferred with `iroh-blobs`, which verifies each chunk as it arrives. Blobs in encrypted conversations are encrypted before hashing, so whoever stores or serves them can't read them.

This replaces Fluxer's upload pipeline (the media proxy's upload relay, signed attachment URLs and the S3 buckets; [cut-lines §3](../cut-lines.md)).

## Uploading

"Uploading" means storing locally, then making the blob available:

1. **Encrypt** the file, unless it's for an unencrypted channel or a public avatar (below).
2. **Hash** the result with BLAKE3. This is the blob hash.
3. **Store** it in the node's blob store, and **pin** it at providers (below).
4. **Send** an attachment reference inside the message. The reference carries the hash, the key and the metadata.

Nothing is sent to a server first. The message and the blob travel separately: the message by envelope, the blob by `iroh-blobs` when a recipient asks for it.

## Encryption

Each blob gets a fresh random 32-byte key. Because the key is random, the same file uploaded twice produces different ciphertexts and different hashes. Nobody can confirm that a particular known file was shared by looking up its hash.

Format: ChaCha20-Poly1305 in 64 KiB chunks, using the STREAM construction:

```
plaintext  = chunks of 65 536 bytes (the last may be shorter, but not empty unless the file is empty)
nonce(i)   = prefix (7 random bytes) ‖ uint32-be(i) ‖ last-flag (0x01 for the final chunk, else 0x00)
ciphertext = ‖ for each i: ChaCha20-Poly1305(key, nonce(i), chunk(i), aad = "NekoChat-v1 blob")
```

- Each ciphertext chunk is 16 bytes longer than its plaintext.
- The last-chunk flag stops anyone truncating the file without detection.
- A receiver can decrypt and show the start of a video while the rest is still downloading.

## Attachment reference

```cddl
blob-ref = {
  "hash": hash,                      ; BLAKE3 of the bytes as stored (ciphertext if encrypted)
  "size": uint,                      ; stored size in bytes
  ? "enc": {
    "key": bstr .size 32,
    "prefix": bstr .size 7,
    "plain-size": uint,
    "plain-hash": hash,              ; BLAKE3 of the plaintext, checked after decrypting
  },
  "providers": [+ pubkey],           ; node IDs to fetch from, in order of preference
}

attachment-ref = {
  "blob": blob-ref,
  "name": tstr .size (1..255),
  "mime": tstr,
  ? "width": uint,
  ? "height": uint,
  ? "duration-ms": uint,
  ? "thumb": bstr .size (0..16384),  ; inline WebP thumbnail, encrypted under the same key with chunk index 0xFFFFFFFF
  ? "thumb-blob": blob-ref,          ; larger preview, when 16 KiB isn't enough
  ? "alt": tstr .size (0..1500),
  ? "spoiler": bool,
  ? "waveform": bstr,                ; voice messages
}

embed = {                            ; a link preview, built by the sender's node
  "url": tstr,
  ? "title": tstr,
  ? "description": tstr,
  ? "site": tstr,
  ? "image": attachment-ref,
  ? "color": uint,
}
```

Because the reference travels inside the encrypted message, only group members learn the key.

## Providers and fetching

A **provider** is a node that holds the blob and will serve it:
- the sender's own device
- the sender's home instance
- a community's home node

The sender lists them in `providers`.

1. The receiver fetches from the first reachable provider with `iroh-blobs`, verifying each chunk against `hash`.
2. If `enc` is present, it decrypts and checks `plain-hash`.
3. A receiver MAY then become a provider itself, but it isn't required to.

Providers SHOULD serve a blob only to peers that are entitled to it:
- for DM attachments, the sender's contacts
- for channel attachments, community members
- for avatars and public listings, anyone

Encrypted blobs are useless without their key, so this is defence in depth, not the main protection. It also limits who can measure what's being shared.

## Pinning and retention

- A device that sends an attachment SHOULD ask a provider that's always online to **pin** it. That's normally its home instance, or the community's home node for channels. Then the attachment stays available after the sender's device goes offline.
- Instances set their own pin quotas per user and per community.
- When a message is deleted, every pinning node MUST unpin its blobs, unless another live message still references the same hash.
- Unpinned blobs MAY be garbage-collected after 7 days.

Pin and unpin requests:

```cddl
; community.blob.pin / unpin (to a home node) or account.blob.pin / unpin (to your own instance)
pin-params = { "blob": blob-ref, "for": neko-id }        ; the message the blob belongs to
unpin-params = { "hash": hash, "for": neko-id }
```

## Unencrypted blobs

These are stored as plain bytes, with no `enc` field:
- attachments in unencrypted community channels
- avatars and banners in public directory listings ([07](07-discovery-and-federation.md))
- community icons

The home node may then make thumbnails and run them through moderation tools, as Fluxer's media proxy does today.

## Thumbnails and link previews

- **Thumbnails** are made by the sender's node before encrypting. The receiver can't ask a server to resize an image it can't read.
- **Link previews** are fetched by the sender's node. Fluxer's unfurler (`fluxer_unfurl`) becomes a library the node calls. The preview travels in the message as an `embed`, so receivers never contact the linked site. This avoids revealing the receiver's IP address.

## Open questions

- **Huge files:** whether to cap blob size per instance only, or set a protocol-wide limit.
- **Re-sharing:** whether forwarding an attachment should reuse the same blob and key (cheap), or re-encrypt it (unlinkable).
