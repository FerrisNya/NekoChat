# Rebrand placeholders

Phase 0, step 1 swapped Fluxer's network and OS identifiers for NekoChat ones. No real NekoChat domain exists
yet, so every endpoint now uses an RFC 2606 reserved domain under `.example`. Those domains never resolve,
which means nothing silently talks to Fluxer's servers. To go live, search-and-replace each placeholder
with a domain you own.

## Domains

Fluxer used several domains. Each one got its own placeholder so they stay distinct (tests depend on that).
You can map several placeholders to one real domain later.

| Placeholder | Replaced | Used for |
|---|---|---|
| `nekochat.example` | `fluxer.app`, `fluxer.chat` | app, API, gateway, media, CDN, docs, admin (`web.`, `api.`, `gateway.`, `media.`, `cdn.`, `canary.` …) |
| `nekochat-site.example` | `fluxer.com` | marketing site, `pkgs.` (desktop update feeds), `push.` (push relay), `feedback.`, support/appeals email |
| `nekochat-media.example` | `fluxer.media` | media CSP entries |
| `static.nekochat.example` | `fluxerstatic.com` | static asset CDN |
| `status.nekochat.example` | `fluxerstatus.com` | status page |
| `nekochat-usercontent.example` | `fluxerusercontent.com` | user content (tests) |
| `nekochat-invite.example` | `fluxer.gg` | invite links |
| `docs.nekochat.example` | `fluxer.dev` (docs domain constant) | docs |
| `translate.nekochat.example` | `weblate.fluxer.tools` | translation platform link |
| `s3.nekochat-storage.example` | `ewr1.vultrobjects.com` | default S3 endpoint (bucket names unchanged) |

## App and package identity

| Item | Now |
|---|---|
| Linux / Flatpak app IDs | `app.nekochat.NekoChat`, `app.nekochat.NekoChatCanary`; desktop entry IDs `app.nekochat.NekoChatDesktop[Canary\|Development]`; packaging files renamed to match |
| macOS bundle IDs | `app.nekochat`, `app.nekochat.canary`, `app.nekochat.development` |
| Product names | `NekoChat`, `NekoChat Canary`, `NekoChat Development`; app default `PRODUCT_NAME` and instance default branding `NekoChat` |
| Deep-link scheme | `nekochat://` (`nekochat-development://` for dev builds) |
| Windows | Velopack IDs `nekochat_desktop[_canary\|_development]`, AppUserModelIDs `NekoChat.NekoChat[.Canary\|.Development]`, freshly generated toast-activator CLSIDs, shortcut/author `NekoChat` |
| Linux packages | `nekochat`, `nekochat-canary`, `nekochat-development`; user-data dirs `nekochat`, `nekochatcanary`, `nekochatdevelopment` |
| Container images | `ghcr.io/ferrisnya/nekochat-*` (`nekochat-api`, `nekochat-gateway`, … in CI, compose, Helm, the image-set tool and Dockerfile title labels) |
| Release tags | image-set releases are `nekochat-release@<version>` |
| Helm chart | chart name `nekochat` (folder and template helpers still `fluxer`) |
| API docs | OpenAPI title `NekoChat API` |
| Web app | page title, PWA manifest name and Apple web-app title `NekoChat`; admin panel `NekoChat Admin` |
| Source links / release repo | `github.com/FerrisNya/NekoChat`; workflow app-token owner `FerrisNya`; CODEOWNERS `@FerrisNya` |

NekoChat now uses its own app ID, deep-link scheme, Windows IDs and data directory, so it can be installed
next to Fluxer without the two colliding. Its "legacy install cleanup" code also no longer targets Fluxer's
old packages.

## Still needs your own values

| Item | Where | Why |
|---|---|---|
| Apple team ID | `fluxer_desktop/build_resources/entitlements.mac.*.plist`, `tools/ci/src/desktop.rs` (`XXXXXXXXXX`) | Fluxer's team ID was removed |
| Apple provisioning profiles | `fluxer_desktop/build_resources/profiles/Fluxer*.provisionprofile` | these are Fluxer's and won't sign NekoChat |
| Windows code-signing subject | `tools/ci/src/desktop.rs` `FLUXER_WINDOWS_SIGNER_COMMON_NAME` (`Fluxer Platform AB`) | must match your certificate |
| Mobile app links | `fluxer_app_proxy/src/routes/{android,apple}_association.rs` | now empty; previously trusted Fluxer's signed apps |
| GitHub App secrets for CI | `.github/workflows/*` | the workflows expect a GitHub App installed on `FerrisNya` |
| macOS 26 dev icon catalog | `fluxer_desktop/build_resources/icons-*/_compiled/Assets.car` | still Fluxer's; rebuild from `AppIcon.icon` with Xcode's `actool` (only the development macOS build ships it) |

## Intentionally unchanged

- **Internal names:** `@fluxer/*` packages, `fluxer_*` folders, `FLUXER_*` env vars (including `FLUXER_REGISTRY*`), CI secret names (`FLUXER_CI_APP_ID`/`_KEY`), Helm template helpers and resource names, compose service names, binary and crate names, asset file names like `fluxer-logo-color.svg`. Users never see these. They get renamed in [roadmap Phase 9](roadmap.md#phase-9-internal-rename-12-weeks), after the old backend is removed.
- **Upstream material:** `README.md`, `fluxer_docs/`, `.github/*.md` (Fluxer's governance/contributing/security policy), license and NOTICE files, translation `.po` headers, the Electron patch author line.
- **Workflows guarded by `github.repository == 'fluxerapp/fluxer'`:** labeller, lock-closed-conversations, Dart SDK dispatch. These stay skipped in the fork.
- **UI text:** the ~400 literal "Fluxer" mentions in app strings and their translations. That's a translation-level change best done with an i18n pass.
- **Brand accent colour:** `#4641D9` (Fluxer blue) is still the default accent, `theme-color` and splash colour. Only icon backgrounds changed.
- **Other artwork:** badges, default avatars, screenshots in `fluxer_docs/` (still Fluxer's, CC BY-SA 4.0).

## Logo

The NekoChat logo replaced the Fluxer logo everywhere it appeared. `NCLogo.svg` (cat) and `NCBanner.svg` (cat + wordmark) are
vector traces of `NCLogo.png` and `NCBanner.png`. The wordmark is traced from the banner, so it keeps the custom lettering.

| Where | Look |
|---|---|
| Stable desktop icons, favicons, Android/Apple/Windows tile icons, Linux icon, in-app symbol | purple cat on white (was white on blue) |
| Canary desktop and Linux icons | white cat on orange (unchanged background) |
| Development desktop icons | white cat on dark (unchanged background) |
| macOS tray template | black cat outline |
| Wordmarks and lockups (`fluxer-wordmark.svg`, `fluxer-logo-wordmark-monochrome.svg`, `logo-color.svg`, `logo-white.svg`, desktop title bar) | banner lettering |
| Windows tile colour | `#FFFFFF` |

License notes in `fluxer_static/LICENSE` and the `NOTICE.md` files say the NekoChat logo is NekoChat contributors' and not
covered by Fluxer's CC BY-SA 4.0 artwork license. No license has been chosen for it yet.
