# Builds the zman sync server (crates/server) into a small image.
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p zman-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home zman
COPY --from=build /src/target/release/zman-server /usr/local/bin/zman-server
USER zman
EXPOSE 8787
CMD ["zman-server"]
