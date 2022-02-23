build-x64:
	cargo build --release

build-aarch64:
	cargo build --release --target aarch64-unknown-linux-musl

build-armhf:
	cargo build --release --target armv7-unknown-linux-musleabihf
install-x64:
	install -T target/release/hpg /usr/local/bin/hpg
