.PHONY: build test bench lint fmt clean wasm install dev coverage audit help

# Default target
.DEFAULT_GOAL := help

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
BLUE := \033[0;34m
NC := \033[0m # No Color

help: ## Show this help message
	@echo "$(BLUE)RuVector Mathpix - Development Commands$(NC)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}'

build: ## Build the project in release mode
	@echo "$(BLUE)Building RuVector Mathpix...$(NC)"
	cargo build --release --all-features

build-dev: ## Build the project in development mode
	@echo "$(BLUE)Building RuVector Mathpix (dev)...$(NC)"
	cargo build --all-features

test: ## Run all tests
	@echo "$(BLUE)Running tests...$(NC)"
	cargo test --all-features --verbose

test-unit: ## Run unit tests only
	@echo "$(BLUE)Running unit tests...$(NC)"
	cargo test --lib --all-features

test-integration: ## Run integration tests only
	@echo "$(BLUE)Running integration tests...$(NC)"
	cargo test --test '*' --all-features

test-doc: ## Run documentation tests
	@echo "$(BLUE)Running documentation tests...$(NC)"
	cargo test --doc

bench: ## Run benchmarks
	@echo "$(BLUE)Running benchmarks...$(NC)"
	cargo bench --all-features

bench-baseline: ## Run benchmarks and save as baseline
	@echo "$(BLUE)Running benchmarks (saving baseline)...$(NC)"
	cargo bench --all-features -- --save-baseline main

bench-compare: ## Compare benchmarks against baseline
	@echo "$(BLUE)Comparing benchmarks against baseline...$(NC)"
	cargo bench --all-features -- --baseline main

lint: ## Run linting checks
	@echo "$(BLUE)Running linting...$(NC)"
	cargo clippy --all-features --all-targets -- -D warnings
	cargo fmt --check

fmt: ## Format code
	@echo "$(BLUE)Formatting code...$(NC)"
	cargo fmt

fix: ## Auto-fix linting issues
	@echo "$(BLUE)Auto-fixing linting issues...$(NC)"
	cargo clippy --all-features --all-targets --fix --allow-dirty --allow-staged
	cargo fmt

clean: ## Clean build artifacts
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	cargo clean
	rm -rf target/
	rm -rf pkg/
	rm -rf node_modules/

wasm: ## Build WebAssembly package
	@echo "$(BLUE)Building WebAssembly package...$(NC)"
	wasm-pack build --target web --features wasm

wasm-test: ## Test WebAssembly package
	@echo "$(BLUE)Testing WebAssembly package...$(NC)"
	wasm-pack test --headless --firefox --chrome

install: ## Install development dependencies
	@echo "$(BLUE)Installing development dependencies...$(NC)"
	rustup update stable
	rustup component add rustfmt clippy
	cargo install cargo-tarpaulin cargo-audit cargo-deny cargo-license
	cargo install wasm-pack
	@echo "$(GREEN)Development environment ready!$(NC)"

dev: ## Setup complete development environment
	@echo "$(BLUE)Setting up development environment...$(NC)"
	./scripts/setup_dev.sh
	@echo "$(GREEN)Development environment setup complete!$(NC)"

coverage: ## Generate code coverage report
	@echo "$(BLUE)Generating coverage report...$(NC)"
	cargo tarpaulin --all-features --out Html --output-dir coverage
	@echo "$(GREEN)Coverage report generated at coverage/index.html$(NC)"

coverage-ci: ## Generate coverage for CI (XML format)
	@echo "$(BLUE)Generating coverage for CI...$(NC)"
	cargo tarpaulin --all-features --out Xml --output-dir coverage --fail-under 80

audit: ## Run security audit
	@echo "$(BLUE)Running security audit...$(NC)"
	cargo audit
	cargo deny check

doc: ## Generate documentation
	@echo "$(BLUE)Generating documentation...$(NC)"
	cargo doc --all-features --no-deps --open

doc-private: ## Generate documentation including private items
	@echo "$(BLUE)Generating documentation (with private items)...$(NC)"
	cargo doc --all-features --no-deps --document-private-items --open

check: ## Run all checks (lint, test, audit)
	@echo "$(BLUE)Running all checks...$(NC)"
	@make lint
	@make test
	@make audit
	@echo "$(GREEN)All checks passed!$(NC)"

ci: ## Run CI pipeline locally
	@echo "$(BLUE)Running CI pipeline...$(NC)"
	@make lint
	@make test
	@make bench
	@make wasm
	@make coverage-ci
	@make audit
	@echo "$(GREEN)CI pipeline completed!$(NC)"

release: ## Build optimized release binary
	@echo "$(BLUE)Building optimized release...$(NC)"
	RUSTFLAGS="-C target-cpu=native" cargo build --release --all-features
	strip target/release/libruvector_scipix.*
	@echo "$(GREEN)Release binary ready at target/release/$(NC)"

profile: ## Run performance profiling
	@echo "$(BLUE)Running performance profiling...$(NC)"
	cargo build --profile bench
	perf record -g target/release/scipix-benchmark
	perf report

flamegraph: ## Generate flamegraph
	@echo "$(BLUE)Generating flamegraph...$(NC)"
	cargo flamegraph --bench scipix_benchmark -- --bench
	@echo "$(GREEN)Flamegraph generated at flamegraph.svg$(NC)"

models: ## Download ONNX models
	@echo "$(BLUE)Downloading ONNX models...$(NC)"
	./scripts/download_models.sh
	@echo "$(GREEN)Models downloaded to models/$(NC)"

watch: ## Watch for changes and rebuild
	@echo "$(BLUE)Watching for changes...$(NC)"
	cargo watch -x build

watch-test: ## Watch for changes and run tests
	@echo "$(BLUE)Watching for changes and running tests...$(NC)"
	cargo watch -x test

update: ## Update dependencies
	@echo "$(BLUE)Updating dependencies...$(NC)"
	cargo update
	@echo "$(GREEN)Dependencies updated!$(NC)"

outdated: ## Check for outdated dependencies
	@echo "$(BLUE)Checking for outdated dependencies...$(NC)"
	cargo outdated

tree: ## Show dependency tree
	@echo "$(BLUE)Dependency tree:$(NC)"
	cargo tree --all-features

bloat: ## Analyze binary size
	@echo "$(BLUE)Analyzing binary size...$(NC)"
	cargo bloat --release --crates

times: ## Show compilation times
	@echo "$(BLUE)Compilation times:$(NC)"
	cargo build --release --timings

verify: ## Verify project is ready for commit
	@echo "$(BLUE)Verifying project...$(NC)"
	@make fmt
	@make lint
	@make test
	@make doc
	@echo "$(GREEN)Project verified and ready for commit!$(NC)"
