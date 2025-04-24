# Stage 1: Build the application
FROM ubuntu:22.04 AS builder

# Avoid interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install necessary dependencies: Rust, build tools, git, ssl, AND Python3
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates curl build-essential pkg-config libssl-dev git \
    python3 python3-dev && \
    rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Create a working directory
WORKDIR /app

# Copy Cargo files and build dependencies first to leverage Docker cache
COPY Cargo.toml Cargo.lock ./
# Create dummy source file to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release --bin suimodular || true
RUN rm -rf src

# Copy the entire project source code
COPY . .

# Build the application in release mode
# Ensure the correct binary name matches your Cargo.toml ([package].name)
RUN cargo build --release --bin suimodular


# Stage 2: Create the final runtime image
FROM ubuntu:22.04 AS runner

# Install only runtime dependencies: ca-certs, libssl, AND Python runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates libssl-dev libpython3.10 && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user to run the application
RUN useradd -ms /bin/bash appuser
USER appuser

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/suimodular .

# Set the entrypoint for the container
ENTRYPOINT ["./suimodular"]