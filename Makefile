all: build

build:
	@cargo build --release

clean:
	@cargo clean

check:
	@cargo check

test:
	@cargo test

fmt:
	@cargo +nightly fmt

lint:
	@cargo clippy --all-targets --all-features -- -D warnings

tidy:
	@cargo clippy --fix --allow-dirty --allow-staged && cargo +nightly fmt 

docker:
	@docker buildx build --tag 'signet:latest' --load .

pecorino:
	@docker buildx build --tag 'pecorino-signet:latest' --load --file Dockerfile.pecorino .

dev:
	@docker compose up

run:
	@cargo run --bin signet

.PHONY: all build run clean check test fmt lint build-cross build-docker docker
