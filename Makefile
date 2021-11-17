WASM_SUBDIRS := $(wildcard ./wasm_test/*/.)

help: ## Display this help screen
	@grep -h -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

all: $(SUBDIRS)

test: ## Test with all features
	cargo test --all-features

wasm: $(WASM_SUBDIRS) ## Compiles for WASM

$(WASM_SUBDIRS):
	$(MAKE) -C $@

.PHONY: help all test wasm $(WASM_SUBDIRS)
