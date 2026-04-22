.PHONY: install uninstall build release help

## build: Build debug binary
build:
	cargo build

## release: Build optimized release binary
release:
	cargo build --release

## install: Install nuke globally via cargo
install:
	cargo install --path .

## uninstall: Uninstall nuke
uninstall:
	cargo uninstall nuke

## help: Show available targets
help:
	@grep -E '^## ' Makefile | sed 's/## /  /'
