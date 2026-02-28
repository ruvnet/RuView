# WiFi-DensePose Makefile
# ============================================================

.PHONY: verify verify-verbose verify-audit help

# Trust Kill Switch -- one-command proof replay
verify:
	@./verify

# Verbose mode -- show detailed feature statistics and Doppler spectrum
verify-verbose:
	@./verify --verbose

# Full audit -- verify pipeline + scan codebase for mock/random patterns
verify-audit:
	@./verify --verbose --audit

help:
	@echo "WiFi-DensePose Build Targets"
	@echo "============================================================"
	@echo ""
	@echo "  make verify          Run the trust kill switch (proof replay)"
	@echo "  make verify-verbose  Verbose mode with feature details"
	@echo "  make verify-audit    Full verification + codebase audit"
	@echo "  make help            Show this help"
	@echo ""
