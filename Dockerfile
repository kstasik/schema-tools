FROM clux/muslrust:stable as builder
WORKDIR /volume
COPY . .
RUN cargo build --release --bin schematools-cli

FROM alpine
COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/schematools-cli .
ENTRYPOINT [ "/schematools-cli" ]
