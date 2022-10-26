build: build-ui
	cargo build

build-ui:
	#!/usr/bin/env bash
	mkdir -p $out
	pushd script || exit
	if [[ ! -d node_modules ]]; then
		yarn install
	fi
	yarn build
	popd || exit
	cp static/* $out/

check: build
	cargo clippy
	cargo test

run: build-ui
	cargo run -- --rp-id localhost --rp-origin http://localhost:8080 --session-secret=$(openssl rand -hex 64)
