# Use the official Rust image as a builder
FROM docker.io/library/rust:1.92 AS builder

# Create a new empty shell project
WORKDIR /usr/src/app

# Copy manifests
COPY Cargo.toml ./
COPY backend/Cargo.toml ./backend/Cargo.toml

# Create dummy main.rs and lib.rs for dependency caching
RUN mkdir -p backend/src/bin && \
    echo "fn main() {}" > backend/src/main.rs && \
    echo "" > backend/src/lib.rs && \
    echo "fn main() {}" > backend/src/bin/cron_runner.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --release --bin backend && \
    rm -rf backend/src

# Copy actual source code
COPY backend/src ./backend/src

# Touch to invalidate cache and rebuild with actual source
RUN touch backend/src/main.rs && \
    cargo build --release --bin backend --bin cron_runner

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
