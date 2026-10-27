# Cut-line map: messaging and uploads

Phase 0 groundwork for replacing NekoChat's centralized messaging/upload backend (inherited from Fluxer)
with a hybrid P2P + federated node. This map records **where messaging and uploads live today and what
touches them**. It deliberately makes no keep/adapt/replace calls: fill in the **Decision** column
(`keep` / `adapt` / `replace` / `remove`) as you go.

Paths are relative to the repo root. `API` = `fluxer_api/src/api`, `APP` = `fluxer_app/src/features`.

## 1. How a message flows today

```
client ── POST /channels/:id/messages ──► fluxer_api (MessageController → MessageSendService)
                                              │ ids:      NATS svc.snowflakes
                                              │ store:    messages / channel_message_buckets (Postgres-KV or Cassandra)
                                              │ render:   NATS svc.messages (BuildResponse)
                                              │ fan-out:  NATS rpc.gateway.guild.dispatch | presence.dispatch
                                              │ async:    JetStream jobs (ExtractEmbeds → svc.unfurl, IndexChannelMessages → Meilisearch, HandleMentions)
                                              ▼
                                       fluxer_gateway (Erlang) ── WebSocket MESSAGE_CREATE ──► clients
                                              │ offline recipients
                                              ▼
                                       NATS push.job.message ──► fluxer_push ──► APNs / FCM / Web Push / relay
```

Uploads: `POST /channels/:id/attachments` returns presigned S3 PUTs, either direct or rewritten through the
media-proxy upload relay. The client PUTs the parts, then calls `POST .../attachments/complete`. When the
message is sent, the API sniffs, hashes, virus-scans and strips metadata, and copies the file from the
`uploads` bucket to the `cdn` bucket. The media proxy serves signed `/attachments/...` URLs.

## 2. fluxer_api: message domain

The message domain lives under `API/channel/`. `API/message/` only holds tests.

### Routes

| Controller | Routes | Decision |
|---|---|---|
| `API/channel/controllers/MessageController.ts` | `GET/POST /channels/:id/messages`, `GET/PATCH/DELETE .../messages/:mid`, `POST /channels/messages/bulk`, `POST .../attachments`, `POST .../attachments/complete`, `DELETE .../messages/:mid/attachments/:aid`, `bulk-delete`, `purge`, `bulk-delete-mine`, `POST .../typing`, `crosspost`, `crosspost-source`, `POST .../messages/:mid/ack`, `DELETE .../messages/ack` | |
| `API/channel/controllers/MessageInteractionController.ts` | pins (`GET .../messages/pins`, `PUT/DELETE .../pins/:mid`, `POST .../pins/ack`), reactions (`GET/PUT/DELETE .../reactions/...`) | |
| `API/channel/controllers/ThreadController.ts`, `ThreadMemberSettingsController.ts` | thread create, list (active/archived), thread-members CRUD | |
| `API/channel/controllers/ForumController.ts` | `threads/search`, `post-data`, tags | |
| `API/channel/controllers/ChannelFollowController.ts` | crosspost followers | |
| `API/read_state/ReadStateController.ts` | `POST /read-states/ack`, `/read-states/ack-bulk` | |
| `API/search/controllers/SearchController.ts` | `POST /search/messages` | |
| `API/user/controllers/UserContentController.ts` | `/users/@me/mentions[...]`, `/users/@me/saved-messages[...]` | |
| `API/user/controllers/UserAccountController.ts` | `preload-messages`, `channels/messages/preload`, `users/@me/messages/delete`, `bulk-delete-mine` | |
| `API/webhook/WebhookController.ts` | `POST /webhooks/:id/:token` (+ github/slack/instatus/libredesk), webhook message GET/PATCH/DELETE | |
| `API/admin/controllers/MessageAdminController.ts` | `/admin/messages`, shreds, per-channel and per-user message admin | |
| `API/attachment/AttachmentController.ts` | `POST /attachments/refresh-urls` | |

### Services

| Area | Files (under `API/channel/services/`, unless the path says otherwise) | Decision |
|---|---|---|
| Facades | `MessageService.ts`, `ChannelService.ts` (`channelService.messages/.attachments/.interactions`) | |
| Request parsing | `message/MessageRequestService.ts`, `MessageRequestParser.ts`, `MessageRequestCompatibility.ts` | |
| Create | `message/MessageSendService.ts`, `MessagePersistenceService.ts`, `MessageProcessingService.ts`, `MessageValidationService.ts`, `MessageContentService.ts`, `MessageStickerService.ts`, `MessageWriteLock.ts` | |
| Edit / delete | `message/MessageEditService.ts`, `MessageDeleteService.ts`, `UserMessageDeletionService.ts`, `SelfMessageFilter.ts` | |
| Read | `message/MessageRetrievalService.ts`, `MessageHistoryCutoff.ts`, `MessageResponseDataService.ts` (→ `svc.messages`), `ThreadMessageResponses.ts` | |
| Mentions / system | `message/MessageMentionService.ts`, `MessageSystemService.ts` | |
| Crosspost | `message/MessageCrosspostService.ts`, `CrosspostDeliveryService.ts`, `CrosspostPropagation.ts`, `CrosspostSourceService.ts`, `CrosspostEmbedObjects.ts` | |
| Embeds | `message/MessageEmbedAttachmentResolver.ts`, `API/infrastructure/EmbedService.ts` | |
| Helpers | `message/MessageHelpers.ts`, `MessageOperationsHelpers.ts`, `MessageChannelAuthService.ts`, `DmScopeUtils.ts`, `MessageActivity.ts`, `ThreadMessageActivity.ts`, `ReadStateHelpers.ts` | |
| Gateway fan-out | `message/MessageGatewayDispatch.ts`, `ChannelGatewayDispatch.ts` | |
| Reactions / pins / typing | `interaction/MessageReactionService.ts`, `MessagePinService.ts`, `MessageReadStateService.ts` (sends `TYPING_START`), `MessageInteractionAuthService.ts`, `MessageInteractionService.ts` | |
| Threads | `thread/*` (~20 files) | |
| Search | `message/MessageSearchService.ts` | |
| Models | `API/channel/MessageTypes.ts`, `EmbedTypes.ts`, `AttachmentDTOs.ts`, `API/models/{Message,Attachment,MessageRef,MessageSnapshot,SavedMessage,RecentMention}.ts` | |

### Repositories and tables

| Repository | Tables | Decision |
|---|---|---|
| `API/channel/repositories/MessageRepository.ts` (facade) + `message/MessageDataRepository.ts`, `MessageDeletionRepository.ts`, `BucketScanEngine.ts` | `messages`, `channel_message_buckets`, `channel_empty_buckets`, `channel_state`, `channel_pins`, `attachment_lookup` | |
| `message/MessageAuthorRepository.ts` | `messages_by_author_id_v2` | |
| `MessageInteractionRepository.ts` | `message_reactions`, `channel_pins` | |
| `CrosspostedMessageRepository.ts` | `crossposted_messages`, `crosspost_sources_by_channel` | |
| `ThreadRepository.ts` | `thread_state`, `thread_stats`, `threads_by_parent`, `active_threads_by_guild`, `archived_threads_by_parent`, `thread_members(_by_user)`, `thread_parent_config`, `forum_pinned_thread`, `guild_thread_state`, `thread_only_channels_by_guild` | |
| `API/read_state/ReadStateRepository.ts` | `read_states` | |
| `API/user/repositories/RecentMentionRepository.ts`, `SavedMessageRepository.ts` | `recent_mentions(_by_guild)`, `saved_messages` | |
| `message/AttachmentUploadTraceRepository.ts` | `attachment_upload_traces_by_key/_by_attachment` | |
| `API/attachment/AttachmentDecayRepository.ts` | `attachment_decay_by_id/_by_expiry` | |

Table definitions are in `API/Tables.ts` (DSL: `API/database/CassandraTableDsl.ts`). The Postgres DDL is
created at runtime in **two places that must stay in sync**: `fluxer_svc/src/postgres.rs` and
`API/database/PostgresKvQueryExecutor.ts`. The schema is one generic `fluxer_kv` table plus partial indexes
for `messages` and `message_reactions`.

## 3. fluxer_api: uploads and attachments

| Piece | Files | Decision |
|---|---|---|
| Multipart (form) upload | `API/channel/services/AttachmentUploadService.ts` (`uploadFormDataAttachments`) | |
| Presigned upload + multipart complete | same file (`requestPresignedAttachmentUploadUrls`, `completeMultipartAttachmentUploads`), `API/infrastructure/StorageService.ts` (AWS SDK) | |
| Upload relay token issuing (GeoIP-based) | `API/channel/services/UploadRelay.ts` | |
| On-send processing (sniff, SHA-256 ban check, ClamAV, metadata strip, uploads→cdn copy) | `API/channel/services/message/AttachmentProcessingService.ts`, `API/infrastructure/MediaService.ts`, `ContentModerationService.ts`, `VirusScanService.ts`, `fluxer_api/pkgs/virus_scan/` | |
| URL signing | `API/attachment/AttachmentUrls.ts` → `fluxer_api/pkgs/media_proxy_utils/src/AttachmentUrlSignature.ts` (TTL 86400 s, 43200 s bucket); Rust twin `fluxer_common/src/attachment_url_signature.rs` | |
| Attachment decay | `API/attachment/AttachmentDecayService.ts`, `AttachmentDecayReconcileRun.ts`, worker `ExpireAttachments` (cron 12 h), `ReconcileAttachmentDecay` | |
| Harvests / data exports | routes under `/users/@me/harvest...`, `/harvest-downloads/:id`; worker `HarvestUserData.ts`, `HarvestGuildData.ts`, `worker/utils/Archive*.ts`, `ContentAddressedAttachmentCollector.ts`; table `user_harvests` | |
| Stream previews | `API/channel/services/StreamPreviewService.ts` (`uploads` bucket) | |

**Non-message assets use a separate path.** Avatars, emoji, stickers, banners, themes, entrance sounds and
favorite memes are sent as base64 in a JSON body and handled by `API/infrastructure/AvatarService.ts` and
`EntityAssetService.ts`, which write to the `cdn` bucket. Callers include `guild/services/content/EmojiService.ts`,
`StickerService.ts`, the user profile services, `oauth/ApplicationService.ts`, `webhook/WebhookService.ts`,
`theme/ThemeService.ts`, `user/entrance_sound/`, `favorite_meme/`.

S3 buckets (`Config.s3.buckets`):

| Bucket | Used for |
|---|---|
| `uploads` | staging for message attachments and stream previews |
| `cdn` | final attachments and all non-message assets |
| `harvests` | data exports |
| `reports` | report evidence |

## 4. fluxer_api: read state, search, unfurl, IDs

| Piece | Files | Decision |
|---|---|---|
| Read state | `API/read_state/{ReadStateService,ReadStateRequestService,ReadStateVisibility,ReadStateChannelMeta,ReadStateResponseMapper}.ts`; also read by `rpc/RpcService.ts` (session start, `get_read_state`) | |
| Search (Meilisearch / Elasticsearch) | `API/search/` (`SearchFactory.ts`, `meilisearch/*`, `message/*`, `GlobalSearchService.ts`), `fluxer_api/pkgs/elasticsearch_search/`; Meilisearch indexes `messages`, `threads`; writer jobs `IndexChannelMessages`, `RefreshSearchIndex`, `ExtractEmbeds`, `MessageShred`, `ThreadMaintenanceTasks` | |
| Unfurl | `API/infrastructure/NatsUnfurlerService.ts` (→ `svc.unfurl`), `EmbedService.ts`, `unfurl/UnfurlController.ts` (`POST /unfurl`, debug only) | |
| Snowflake IDs | `API/infrastructure/SnowflakeService.ts` (→ `svc.snowflakes`, batches of 128); local `generateSnowflake()` in `MessageDataRepository.ts`, `EntranceSoundService.ts`. ~45 caller files across admin, auth, channel, guild, user, report, webhook, worker | |

## 5. fluxer_api: gateway dispatch

Transport: `API/infrastructure/GatewayService.ts` → `GatewayRpcClient.ts` → `NatsGatewayRpcTransport.ts`
(NATS request/reply on `rpc.gateway.<method>`). `ChannelGatewayDispatch.dispatchChannelEvent` sends guild
channels through `guild.dispatch` and DMs / group DMs through `presence.dispatch` for each recipient.

| Event | Dispatched from |
|---|---|
| `MESSAGE_CREATE` | `message/MessageGatewayDispatch.ts`, `CallService.ts`, `thread/ThreadCreationService.ts` |
| `MESSAGE_UPDATE` | `MessageGatewayDispatch.ts`, thread services, `rpc/RpcService.ts`, `worker/tasks/ExtractEmbeds.ts`, `ThreadMentionScope.ts` |
| `MESSAGE_DELETE(_BULK)` | `MessageGatewayDispatch.ts`, `CrosspostDeliveryService.ts`, `AdminMessageService.ts`, `AccountStateApplier.ts`, `UserMessageDeletionService.ts`, `MessageShred.ts` |
| `MESSAGE_REACTION_*` | `interaction/MessageReactionService.ts` |
| `CHANNEL_PINS_UPDATE` | `MessagePinService.ts`, `MessageDeleteService.ts` |
| `TYPING_START` | `interaction/MessageReadStateService.ts` |
| `MESSAGE_ACK`, `CHANNEL_PINS_ACK` | `read_state/ReadStateService.ts` |
| `RECENT_MENTION_DELETE`, `SAVED_MESSAGE_*` | `user/services/UserContentService.ts` |
| `THREAD_*` | `thread/ThreadDispatch.ts` |

API → gateway RPCs the message code uses: `guild.resolve_channel_mentions`, `guild.resolve_mention_sources_page`,
`guild.get_user_permissions(_batch)`, `guild.check_permission`, `guild.get_viewable_channels`,
`push.clear_channel_notifications`, `call.*`.

Gateway → API: `POST /internal/rpc` (header `x-fluxer-rpc-auth`), handled by `API/rpc/InternalRpcController.ts` → `RpcService.ts`.
Message-related types: `session` (READY base, including read states), `get_read_state`, `forum_unreads`,
`list_thread_members`, `guild_thread_flip_data`, `get_channel_threads_config`, `call_ended` (upserts the
call message and dispatches `MESSAGE_UPDATE`), plus the push RPCs.

## 6. fluxer_api: what else depends on message/attachment code

| Domain | Coupling |
|---|---|
| admin | `AdminMessageService.ts`, `AdminReportService.ts`, `AdminMessageDeletionService.ts`, `AdminMessageShredService.ts`, `AdminArchiveService.ts`, `AdminSearchService.ts`, `InstanceConfigAdminController.ts` (attachment decay) |
| user | `NewConversationLimit.ts`, `AccountStateApplier.ts`, `UserChannelService.ts`, `UserContentService.ts`, `UserDeletionService.ts` (anonymize/delete messages), `SystemDmMessage.ts` (sends system DMs) |
| guild | `GuildSearchService.ts`, `data/GuildOperationsService.ts` (deletes channel messages + attachments), `member/GuildMemberOperationsService.ts`, `GuildModerationService.ts` |
| webhook | `WebhookController.ts`, `WebhookRequestService.ts`, `WebhookService.ts`, `ChannelFollowService.ts` |
| report / enforcement | `ReportService.ts`, `ReportDeletion.ts`, `EnforcementEvidence.ts`, `EnforcementDecisionService.ts` |
| rpc | `RpcService.ts` |
| favorites | `favorite_meme/*`, `favorite_gif/*` |
| wiring | `middleware/ServiceMiddleware.ts`, `GuildStackServiceFactory.ts`, `ServiceSingletons.ts`, `types/HonoEnv.ts` |
| worker | ~25 message/attachment tasks (bulk deletes, crossposts, embeds, mentions, indexing, shred, decay, harvests); crons in `worker/WorkerMain.ts` |
| bots / oauth / auth | no direct imports found |

Message code's own outbound dependencies: `UserRepository`, `GuildRepository`, `GuildAuditLogService`,
`GuildService`, `limits/*` (attachment size, message limits, premium coupling), `experiment/ChannelThreadsGate`,
`InstanceConfigRepository`, `AccountLimit`, `NewConversationLimit`, `WebhookRepository`, `EmailVerificationUtils`.

### Fluxer business domains (not messaging, listed for completeness)

`API/stripe/`, `API/billing/`, `API/premium/`, `API/store_billing/` (App Store / Google Play), `API/donation/`,
gifts and "Visionary" slots (`models/GiftCode.ts`, `Payment.ts`, `VisionarySlot.ts`, the related user repositories),
plus their worker tasks. `bluesky/` and `connection/` are account integrations.

## 7. fluxer_gateway (Erlang)

| Module(s) under `fluxer_gateway/src/` | Role | Interfaces | Decision |
|---|---|---|---|
| `gateway/gateway_nats_rpc_handler.erl`, `gateway_rpc_router.erl`, `gateway_nats_rpc.erl` | NATS RPC intake, split by role | `rpc.gateway.{guild,voice,process,presence,call,push}.>`, `config.gateway.rollout`, `config.channel.threads` | |
| `gateway/gateway_rpc_guild*.erl` | guild fan-out, visibility, mentions, permissions | `guild.dispatch(_many)`, `guild.get_viewable_channels`, `guild.resolve_*`, `guild.check_permission`, `guild.get_user_permissions(_batch)` | |
| `gateway/gateway_rpc_presence.erl` | per-user fan-out (DMs, acks) | `presence.dispatch`, `presence.sync_group_dm_recipients` | |
| `gateway/gateway_rpc_push.erl` | push settings | `push.sync_user_guild_settings`, `push.clear_channel_notifications` | |
| `guild/guild_dispatch*.erl`, `guild_broadcaster.erl` | filter / decorate / send guild events | internal | |
| `guild/guild_thread_*.erl`, `guild_request_forum_unreads.erl` | thread dispatch, forum unreads | RPC to API | |
| `gateway/gateway_dispatch_relay*.erl` | cross-node relay | internal | |
| `session/session_dispatch.erl`, `session_passive.erl`, `session_ready*.erl` | per-session delivery, replay buffer, READY assembly | API RPC `session` | |
| `presence/presence.erl`, `presence_update.erl` | `message_create` / `message_ack` drive push and read marks | internal | |
| `push/*.erl` (eligibility, preview rendering, outbox) | push decisioning | publishes `push.job.{message,clear,ring}`; RPC to API `get_read_state`, `get_user_blocked_ids`, `get_user_guild_settings` | |
| `utils/event_atoms.erl`, `utils/snowflake_id.erl`, `utils/constants.erl` | event names, snowflake decode, opcodes | — | |

READY contains no message bodies. Messages and typing have no opcodes of their own: they are sent over REST
and come back as dispatches. The message-adjacent opcodes are 14 (lazy request) and 28 (forum unreads).

## 8. Rust services

| Crate | Role | Interface | Storage | Callers | Decision |
|---|---|---|---|---|---|
| `fluxer_svc` | NATS router/shard framework (hash ring), Postgres-KV + Scylla helpers | `svc.<name>` → `svc.<name>.shard.<id>` | `fluxer_kv` | all svc crates | |
| `fluxer_messages` | builds message responses, signs attachment URLs, extracts mentions | `svc.messages`: `GetResponseById`, `BuildResponse(s)`, `ListResponses`, `ExtractMentions` | reads `messages`, `channel_message_buckets`, `message_reactions`, `attachment_decay_by_id`; one delete path (`shard_impl.rs`) | API `MessageResponseDataService.ts` | |
| `fluxer_users` | user partials | `svc.users`: `GetPartialsByIds`, `GetApiPartialsByIds`, `Invalidate` | `users` | messages shard, API | |
| `fluxer_snowflakes` | ID generation | `svc.snowflakes`: `GenerateBatch` | none | API | |
| `fluxer_media_proxy` | attachment + asset serving, upload relay, external media proxy, thumbnails | HTTP: `/attachments/...` (signed), `/external/...`, `PUT /v1/relay/{key}` (HMAC token), internal `/_metadata`, `/_sniff`, `/_thumbnail`, `/_frames`; modes `mp`/`static`/`upload`/`relay` | S3 `cdn`, `uploads` | browsers, API, unfurl | |
| `fluxer_push` | push delivery (APNs, FCM, Web Push, relay) | subscribes `push.job.*`, `config.push.delivery`; HTTP RPC to API | none | gateway | |
| `fluxer_unfurl` | link embeds (HTML, oEmbed, ActivityPub) | `svc.unfurl`: `Unfurl` | memory | API worker | |
| `fluxer_gifs` | Klipy GIF search | `svc.gifs` | memory | API | |
| `fluxer_common` | shared: `attachment_url_signature.rs`, `external_media_path.rs`, `user_partial.rs`, config/http/metrics | — | — | all | |

**Snowflake uniqueness:** IDs are 64-bit (42-bit ms timestamp since `1420070400000`, then a 10-bit worker ID,
then a 12-bit sequence). The worker ID is `FLUXER_SVC_SHARD_ID`, which compose hard-codes to `0`. IDs are
therefore unique only inside one deployment. Two independent instances will generate colliding IDs.
Message storage buckets (10-day windows) are also derived from the ID timestamp: `packages/snowflake/src/SnowflakeBuckets.ts`
and `fluxer_messages` (`BUCKET_DURATION_MS`).

## 9. Infrastructure tied to messaging/uploads

| Kind | Items |
|---|---|
| NATS core | `svc.{messages,users,snowflakes,unfurl,gifs}` (+ `.shard.N`), `rpc.gateway.*`, `push.job.*`, `config.*` |
| JetStream | stream `JOBS` (`jobs.>`), `JOBS_DLQ`; `storage.>` change feed (`StorageChangeFeed.ts`) |
| Valkey keys | `message-nonce:{user}:{nonce}`, `message:{ch}:{mid}:write`, `slowmode(-thread):{ch}:{user}`, `bulk_message_deletion_queue:{user}`, `message_shred_status:{job}`, `mention-chunk:`, `report:message:*` |
| S3 | `attachments/{channelId}/{attachmentId}/{filename}` in `cdn`; random UUID keys in `uploads` |
| Meilisearch | `messages`, `threads` indexes |
| Compose (`deploy/self-hosting/docker-compose.yml`) | `nats`, `meilisearch`, `seaweedfs(-init)`, `media-proxy`, `push`, `{snowflakes,users,gifs,messages,unfurl}(-shard)` |
| Helm (`deploy/helm/fluxer`) | `templates/svc.yaml` (router Deployment + shard StatefulSet per svc), `apps.yaml` (media-proxy, push), `nats/meilisearch/seaweedfs/valkey/postgres.yaml`, `values.yaml` `svc.*`, `mediaProxy`, `push`, `seaweedfs.buckets` |

## 10. Shared contracts

- **Zod schemas** in `packages/schema/src/domains/message/`:
  - `MessageResponseSchemas.ts` (~83 importers)
  - `MessageRequestSchemas.ts`
  - `AttachmentSchemas.ts`
  - `AttachmentUploadSchemas.ts`
  - `EmbedSchemas.ts`
  - `SharedMessageSchemas.ts`
  - `CrosspostSourceSchemas.ts`

  Related schemas: `primitives/MessageValidators.ts`, `FileValidators.ts`, `domains/channel/*`,
  `domains/rpc/RpcSchemas.ts` (`RpcResponseSessionData` = READY base), `domains/gateway/GatewaySchemas.ts`,
  `contracts/search/*`, `branded/WireIds.ts`.
- **Constants** in `packages/constants/src/`:
  - `ChannelConstants.ts` (`MessageTypes`, `MessageFlags`, `MessageStates`, `ChannelTypes`, …; 275 importers)
  - `LimitConstants.ts` (`MAX_ATTACHMENTS_PER_MESSAGE=10`)
  - `GatewayConstants.ts` (opcodes, close codes)
  - `MediaProxyImageSizes.ts`, `MediaProxyAssetSizes.ts`
- **Protobuf** covers UI preferences only (`packages/schema/proto/fluxer/user/preferences/v1/`). No message protos exist.
- **Rust wire types** are hand-written serde enums in each crate's `types.rs`. The API sends JSON to `svc.<name>`.

## 11. fluxer_app (web client)

### Transport and endpoint discovery

| Path | Role | Decision |
|---|---|---|
| `packages/instance_bootstrap/src/Discovery.ts` | fetches `/.well-known/fluxer` (endpoint document) | |
| `packages/instance_bootstrap/src/ClientRuntimeDiscovery.ts`, `EndpointNormalization.ts`, `BuildDiscovery.ts` | endpoint kinds: api, apiPublic, discoveryApi, gateway, media, uploadRelay, staticCdn, webApp, marketing, docs, admin, invite, gift | |
| `packages/instance_bootstrap/src/OfficialInstance.ts`, `OfficialWebAppHosts.ts` | hard-coded official instance (now `nekochat.example` placeholders) | |
| `APP/app/state/InstanceSnapshotStore.ts`, `RuntimeConfig.ts`, `RuntimeConfigSnapshot.ts`, `DesktopRuntimeTransaction.ts`, `APP/app/domain_migration/*` | resolved endpoints, runtime switch | |
| `APP/app/constants/Endpoints.ts` | every REST path | |
| `APP/platform/transport/RestTransport.ts` (`http` singleton: auth, sudo header, 429 pacing, XHR progress) | REST client | |
| `APP/platform/transport/InstanceHTTP.ts`, `APP/platform/DesktopLocalAppRuntime.ts` | `{apiEndpoint}/v{n}{path}`; desktop goes through the local proxy | |
| `fluxer_app/src/app/SetupHttp.ts`, `APP/platform/state/AuthSession.ts` | token install | |

### Gateway client (`APP/gateway/`)

| Path | Role | Decision |
|---|---|---|
| `transport/GatewaySocket.ts` | protocol (HELLO/IDENTIFY/RESUME/HEARTBEAT, …), `zstd-stream` via `GatewayCompression.ts` + libfluxcore WASM | |
| `transport/GatewayWireTransport.ts` | browser WebSocket vs desktop native socket | |
| `transport/GatewaySessionPool.ts`, `PooledGatewayConnection*.ts`, `Foreground*.ts`, `Background*.ts`, `DesktopBackgroundGateway.ts` | multi-account foreground/background sessions | |
| `events/EventRouter.ts`, `events/GatewayReady.ts` | event name → feature handler | |
| `snapshot/*` | READY / dispatch caching (disk-backed on desktop) | |

Message-related handlers:
- `APP/messaging/events/`:
  - `MESSAGE_CREATE`, `MESSAGE_UPDATE`, `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `MESSAGE_ACK`
  - `MESSAGE_REACTION_*`
  - `TYPING_START`
  - `RECENT_MENTION_DELETE`
  - `SAVED_MESSAGE_*`
- `APP/channel/events/`: `CHANNEL_PINS_*`
- `APP/threads/events/ForumUnreads.ts`
- `APP/guild/events/PassiveUpdates.ts`

### Messaging feature

| Area | Files | Decision |
|---|---|---|
| Stores | `APP/messaging/state/` (`MessagingMessages.ts`, `ChannelMessages.ts`, load/incoming state machines, reactions, references, edit, drafts, saved, favorites) | |
| Models | `APP/messaging/models/MessagingMessage.ts`, `UploadingAttachment.ts` | |
| Send queue | `APP/messaging/state/MessageQueue.tsx`, `MessageQueueStateMachine.ts` (nonce, JSON or multipart `payload_json` + `files[n]`) | |
| Optimistic send | `APP/messaging/hooks/useMessageSubmission.tsx`, `utils/MessageSubmitUtils.ts`, `MessageRequestUtils.ts` | |
| REST commands | `APP/messaging/commands/MessageCommands.tsx`, `ReactionCommands.tsx`, `SavedMessageCommands.tsx`, … | |
| Upload pipeline | `APP/messaging/upload/AttachmentUploadPlan.ts` (presigned singlepart/multipart), `ChunkedAttachmentUploader.ts` (4 parallel parts, retry), `CloudUpload.ts`, `APP/channel/components/UploadManager.tsx` | |
| Media URLs | `APP/messaging/utils/DesktopResourceUrl.ts`, `AttachmentCdnUrl.ts`, `state/AttachmentUrlRefresher.ts`, `MediaProxyUtils` | |
| Embeds (display only) | `APP/channel/components/embeds/*`, `APP/messaging/components/embeds/embed_card/*` | |
| Search UI | `APP/search/utils/SearchUtils.ts` (`POST /search/messages`), `SearchQueryParser.ts`, search result components | |
| Read state | `APP/read_state/` (acks, mention and entry state machines) | |
| Mentions | `APP/notification/commands/RecentMentionCommands.ts`, `state/MentionFeed.ts` | |
| Typing | `APP/typing/rolling/RollingTypingSender.ts` (`POST /channels/:id/typing`) | |
| Push | `APP/platform/push/PushSubscriptionService.ts`, `service_worker/WorkerPushPayload.ts` | |

### Snowflake assumptions in the client

`packages/snowflake/src/SnowflakeUtils.ts` (`extractTimestamp`, `fromTimestamp`, `compare`, `sortBySnowflakeDesc`, …)
is imported by 52 non-test files (~71 call sites). These rely on IDs that sort by time and encode their creation timestamp:
- **Ordering and pagination:**
  - `messaging/utils/MessagePaginationUtils.ts`
  - `channel/state/Channels.ts`
  - `read_state/state/read_states/shared.ts`
  - `threads/state/ChannelThreads.ts`
- **Timestamps:**
  - `messaging/utils/MessageGroupingUtils.ts`
  - `user/models/User.ts`
- **Nonces:**
  - `messaging/hooks/useMessageSubmission.tsx` (`fromTimestamp(Date.now())`)
- **Date filters:**
  - `search/utils/SearchQueryParser.ts`

There are also 12 `BigInt(...Id)` uses.

## 12. fluxer_desktop

| Path | Role | Decision |
|---|---|---|
| `native/gateway-socket/` (Rust napi) + `src/main/NativeGatewayTransport*.ts`, `GatewaySocketNativeBoundary.ts`, `src/preload/NativeGatewayPreload.ts`, `packages/desktop_ipc/src/GatewayTransportContract.ts` | native gateway WebSocket bridge | |
| `src/main/LocalAppAPIProxyRequestHandler.ts`, `LocalAppProxyClient.ts`, `LocalAppRemoteResourceRequestHandler.ts`, `LocalAppProtocol*.ts`, `DesktopRuntimeDiscovery.ts`, `SelectedInstanceFetch.ts` | local `app://` proxy to the instance API, media and uploads | |
| `src/main/LocalAppCSP.ts` | CSP allowlist (instance, media, upload hosts) | |
| `src/main/ShellDownloadFormats.ts`, `ModuleManifestRepository.ts`, `ShellSelfUpdate.ts` | update feeds: `{packageOrigin}/desktop/{channel}/{platform}/{arch}/…` (now `pkgs.nekochat-site.example`) | |
