test:
	. "$$HOME/.cargo/env" && cargo test --workspace
	node --test tests/verification/*.test.mjs
