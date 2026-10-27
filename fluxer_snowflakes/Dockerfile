# SPDX-License-Identifier: AGPL-3.0-or-later

FROM rust:1-trixie AS builder

WORKDIR /usr/src/app

COPY . .

RUN cargo build --release -p fluxer-snowflakes

FROM debian:trixie-slim

ARG BUILD_VERSION=""
ARG SOURCE_SHA=""
ARG SOURCE_DATE=""

LABEL org.opencontainers.image.title="nekochat-snowflakes"
LABEL org.opencontainers.image.description="Fluxer snowflake ID service"
LABEL org.opencontainers.image.licenses="AGPL-3.0-or-later"
LABEL org.opencontainers.image.vendor="Fluxer"
LABEL org.opencontainers.image.url="https://nekochat.example"
LABEL org.opencontainers.image.documentation="https://docs.nekochat.example"
LABEL org.opencontainers.image.source="https://github.com/FerrisNya/NekoChat"
LABEL org.opencontainers.image.version="${BUILD_VERSION}"
LABEL org.opencontainers.image.revision="${SOURCE_SHA}"
LABEL org.opencontainers.image.created="${SOURCE_DATE}"
LABEL app.nekochat.build-version="${BUILD_VERSION}"

WORKDIR /usr/local/bin

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
	rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/fluxer-snowflakes /usr/local/bin/fluxer-snowflakes

ENV BUILD_VERSION="${BUILD_VERSION}"

USER 65532:65532

EXPOSE 8090

CMD ["/usr/local/bin/fluxer-snowflakes"]
