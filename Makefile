.PHONY: build build_musl

build:
	cargo build --release

build_musl:
	cross build --target x86_64-unknown-linux-musl --release
