INSTALL_DIR := $(HOME)/.local/bin
BINARIES := cozy czv

.PHONY: install build

build:
	cargo build --release

install: build
	@mkdir -p $(INSTALL_DIR)
	install -m 755 target/release/cozy $(INSTALL_DIR)/cozy
	install -m 755 target/release/czv $(INSTALL_DIR)/czv
