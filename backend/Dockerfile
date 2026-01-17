# Use the official Rust image as a builder
FROM docker.io/library/rust:1.92 AS builder

WORKDIR /usr/src/app

# Copy everything needed for build
COPY Cargo.toml ./
COPY backend ./backend

# Build release binaries
RUN cargo build --release --bin backend --bin cron_runner

# Runtime image
FROM docker.io/library/debian:trixie-slim

RUN apt-get update && \
    apt-get install -y libpq-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/backend /usr/local/bin/backend
COPY --from=builder /usr/src/app/target/release/cron_runner /usr/local/bin/cron_runner

# Create non-root user
RUN useradd -r -s /bin/false appuser
USER appuser

EXPOSE 8080

CMD ["backend"]
