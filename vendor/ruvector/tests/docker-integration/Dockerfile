# Test environment for ruvector-attention published packages
FROM node:20-slim

# Install Rust for testing the crate
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Copy test files
COPY package.json ./
COPY Cargo.toml ./
COPY test-wasm.mjs ./
COPY test-napi.mjs ./
COPY src/ ./src/

# Install npm packages
RUN npm install

# Build and test Rust crate
RUN cargo build --release
RUN cargo test --release

# Run Node.js tests
CMD ["node", "--test"]
