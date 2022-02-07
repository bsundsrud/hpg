build-x64:
	cargo build --release

build-aarch64:
	cargo build --release --target aarch64-unknown-linux-musl

build-armhf:
	cargo build --reledase --target armv7-unknown-linux-musleabihf
