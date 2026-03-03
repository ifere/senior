.PHONY: all daemon daemon-dev extension test test-integration test-all clean

all: daemon extension

daemon:
	cd daemon && cargo build --release

daemon-dev:
	cd daemon && cargo build

extension:
	cd extension && npm install && npm run compile

# Unit tests only — fast, no side effects, no daemon required
test:
	cd extension && npm test
	cd daemon && cargo test --bin senior-daemon

# Integration tests — spawns the real daemon binary, tests real socket I/O
# Requires: daemon binary built (make daemon), sox installed (brew install sox)
test-integration: daemon
	cd daemon && cargo test --test integration_test
	cd extension && npm run test:integration

# Run everything
test-all: test test-integration

clean:
	cd daemon && cargo clean
	cd extension && rm -rf out node_modules
