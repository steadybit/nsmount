# ==================================================================================== #
# HELPERS
# ==================================================================================== #
export PATH := $(HOME)/.cargo/bin:$(PATH)

## help: print this help message
.PHONY: help
help:
	@echo 'Usage:'
	@sed -n 's/^##//p' ${MAKEFILE_LIST} | column -t -s ':' |  sed -e 's/^/ /'

# ==================================================================================== #
# BUILD
# ==================================================================================== #

.PHONY: build
build:
	cross build --release --target x86_64-unknown-linux-gnu
	cross build --release --target aarch64-unknown-linux-gnu
