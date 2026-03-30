FROM debian:bookworm-slim

RUN apt-get update -qq && apt-get install -y \
    libglib2.0-0 \
    libgtk-3-0 \
    libudev1 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY v1/ ./v1/
COPY docs/ ./docs/

EXPOSE 3000

CMD ["echo", "RuView WiFi DensePose - runtime container ready"]