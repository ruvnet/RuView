# Build / sign / upload pipeline for cog-pose-estimation.
# See ADR-100 §"Build pipeline" for the full contract.

CRATE       := cog-pose-estimation
VERSION     := $(shell cargo pkgid -p $(CRATE) 2>/dev/null | sed -E 's/.*#([0-9.]+).*/\1/')
GCS_BUCKET  := gs://cognitum-apps/cogs

ARCHES      := arm x86_64

# --- Build targets ---

.PHONY: build build-arm build-x86_64

build: build-arm build-x86_64

build-arm:
	cargo build -p $(CRATE) --release --target aarch64-unknown-linux-gnu
	cp ../../target/aarch64-unknown-linux-gnu/release/$(CRATE) ./dist/cog-$(CRATE)-arm

build-x86_64:
	cargo build -p $(CRATE) --release --target x86_64-unknown-linux-gnu
	cp ../../target/x86_64-unknown-linux-gnu/release/$(CRATE) ./dist/cog-$(CRATE)-x86_64

# --- Sign ---

.PHONY: sign sign-arm sign-x86_64

sign: sign-arm sign-x86_64

sign-arm: dist/cog-$(CRATE)-arm
	sha256sum dist/cog-$(CRATE)-arm | cut -d' ' -f1 > dist/cog-$(CRATE)-arm.sha256
	# Signature: gcloud secrets versions access latest --secret=COGNITUM_OWNER_SIGNING_KEY \
	#   | openssl pkeyutl -sign -inkey /dev/stdin -rawin -in dist/cog-$(CRATE)-arm.sha256 \
	#   | base64 -w0 > dist/cog-$(CRATE)-arm.sig
	@echo "TODO: wire Ed25519 sign step once COGNITUM_OWNER_SIGNING_KEY is provisioned to CI."

sign-x86_64: dist/cog-$(CRATE)-x86_64
	sha256sum dist/cog-$(CRATE)-x86_64 | cut -d' ' -f1 > dist/cog-$(CRATE)-x86_64.sha256

# --- Upload to GCS ---

.PHONY: upload upload-arm upload-x86_64

upload: upload-arm upload-x86_64

upload-arm: dist/cog-$(CRATE)-arm
	gsutil cp dist/cog-$(CRATE)-arm $(GCS_BUCKET)/arm/cog-$(CRATE)-arm

upload-x86_64: dist/cog-$(CRATE)-x86_64
	gsutil cp dist/cog-$(CRATE)-x86_64 $(GCS_BUCKET)/x86_64/cog-$(CRATE)-x86_64

# --- Manifest ---

.PHONY: manifest

manifest:
	@./scripts/render-manifest.sh $(VERSION)
