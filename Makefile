WASM_TARGET := wasm32v1-none
WASM := target/$(WASM_TARGET)/release/ourdao_dao.wasm

.PHONY: all build test fmt clean optimize

all: build test

build:
	cargo build --target $(WASM_TARGET) --release

# Small, deterministic release wasm ready for deployment.
optimize: build
	stellar contract build --optimize

test:
	cargo test

fmt:
	cargo fmt --all

clean:
	cargo clean
