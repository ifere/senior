.PHONY: all daemon daemon-dev extension test test-integration test-all check-env clean

# CACTUS_LIB_DIR must point to the directory containing libcactus.dylib / libcactus.so.
# Set it in your shell or pass it on the command line:
#   CACTUS_LIB_DIR=/path/to/cactus/build make

check-env:
ifndef CACTUS_LIB_DIR
	$(error CACTUS_LIB_DIR is not set. Example: CACTUS_LIB_DIR=/path/to/cactus/build make)
endif

all: check-env daemon extension

daemon: check-env
	cd daemon && CACTUS_LIB_DIR=$(CACTUS_LIB_DIR) cargo build --release

daemon-dev: check-env
	cd daemon && CACTUS_LIB_DIR=$(CACTUS_LIB_DIR) cargo build

extension:
	cd extension && npm install && npm run compile

# Unit tests only — fast, no side effects, no release binary required
test: check-env
	cd extension && npm test
	cd daemon && CACTUS_LIB_DIR=$(CACTUS_LIB_DIR) cargo test --bin senior-daemon

# Integration tests — spawns the real daemon binary, tests real socket I/O
# Requires: release binary built (make daemon), sox installed (brew install sox)
test-integration: daemon
	cd daemon && cargo test --test integration_test
	cd extension && npm run test:integration

# Run everything
test-all: test test-integration

clean:
	cd daemon && cargo clean
	cd extension && rm -rf out node_modules
