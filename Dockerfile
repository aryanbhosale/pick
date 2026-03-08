FROM rust:slim AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release --locked

FROM debian:bookworm-slim

COPY --from=builder /build/target/release/pick /usr/local/bin/pick

ENTRYPOINT ["pick"]
