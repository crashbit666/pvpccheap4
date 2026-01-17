# Use the official Rust image as a builder
FROM rust:1.92 as builder

# Create a new empty shell project
WORKDIR /usr/src/app

# Copy manifests
COPY Cargo.toml ./
COPY backend/Cargo.toml ./backend/Cargo.toml

# Copy source code
COPY backend/src ./backend/src

# Build
RUN cargo build --release --bin backend

# Runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libpq-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/backend /usr/local/bin/backend

CMD ["backend"]
