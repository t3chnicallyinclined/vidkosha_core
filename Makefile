.PHONY: fmt clippy lint test check smoke bootstrap dataset/export

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features

lint: fmt clippy
	@echo "Lint checks complete."

test:
	cargo test -q

check: fmt clippy test
	@echo "Fmt, clippy, and tests passed."

smoke:
	cargo run -- helix-smoke

bootstrap:
	./scripts/node-operator/bootstrap.sh

dataset/export:
	./scripts/export_helix_finetune_dataset.sh
