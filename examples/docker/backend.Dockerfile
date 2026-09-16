FROM rust:1.95-slim AS build
WORKDIR /src
COPY . /src
RUN cargo build --release --manifest-path examples/Cargo.toml -p jsonapi-showcase-backend

FROM debian:bookworm-slim
# sqlx's sqlite feature bundles libsqlite3 at compile time, so no libsqlite3-0
# runtime package is needed. ca-certificates is included for any outbound TLS.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/examples/target/release/jsonapi-showcase-backend /usr/local/bin/backend
ENV APP_BIND=0.0.0.0:8080
EXPOSE 8080
CMD ["backend"]
