#==================== Builder ====================
FROM rust:1.79-bookworm as builder

WORKDIR /root/app
COPY --chown=root:root . .

RUN cargo build --release --bin star

#==================== Runner ====================
FROM gcr.io/distroless/cc-debian12 as runner

WORKDIR /root/app
COPY --from=builder --chown=root:root /root/app/target/release/star ./star

ENTRYPOINT ["./star"]
