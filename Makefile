.PHONY: all daemon extension clean

all: daemon extension

daemon:
	cd daemon && cargo build --release

daemon-dev:
	cd daemon && cargo build

extension:
	cd extension && npm install && npm run compile

clean:
	cd daemon && cargo clean
	cd extension && rm -rf out node_modules
