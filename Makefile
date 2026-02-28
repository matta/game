.PHONY: check check-fmt check-clippy check-xtask test

check: check-fmt check-clippy check-xtask

check-fmt:
	cargo fmt -- --check

check-clippy:
	cargo clippy --all-targets --all-features -- -D warnings

check-xtask:
	cargo run -p xtask -- check --all

test:
	cargo test $(ARGS)
