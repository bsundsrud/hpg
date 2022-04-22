build-x64:
	cargo build --release

build-aarch64:
	cargo build --release --target aarch64-unknown-linux-musl

build-armv7hf:
	cargo build --release --target armv7-unknown-linux-musleabihf

build-armv6hf:
	PATH="/home/bsundsrud/devel/tools/rpi-tools/arm-bcm2708/arm-linux-gnueabihf/bin:$$PATH" \
	cargo build --release --target arm-unknown-linux-gnueabihf

install-x64:
	install -T target/release/hpg /usr/local/bin/hpg
