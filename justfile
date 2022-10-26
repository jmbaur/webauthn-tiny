export WEBAUTHN_TINY_LOG := "debug"
export ASSETS_DIRECTORY := env_var("out")

build: build-ui
	cargo build

build-ui:
	#!/usr/bin/env bash
	mkdir -p $out
	cd script
	if [[ ! -d node_modules ]]; then
		yarn install
	fi
	yarn build
	cd ..
	cp static/* $out/

check: build-ui
	cargo check
	cargo test

run: build-ui
	cargo run -- --rp-id localhost --rp-origin http://localhost:8080 --session-secret=$(openssl rand -hex 64)
