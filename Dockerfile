#==================== Builder ====================
FROM rust:1.78-bookworm as builder

WORKDIR /root/app
COPY --chown=root:root . .

RUN cargo build --release --bin star

#==================== Runner ====================
FROM gcr.io/distroless/base-debian12 as runner

COPY --from=builder --chown=root:root /root/app/target/release/star /usr/local/bin/star

ENTRYPOINT ["sh",  "-c", "star"]
