export ASSETS_DIRECTORY := env_var("out")

help:
	@just --list

update:
	cargo update
	cargo upgrade
	pushd script && yarn upgrade && popd

build: build-ui
	cargo build

build-ui:
	#!/usr/bin/env bash
	mkdir -p $out
	cd script
	[[ ! -d node_modules ]] && yarn install
	yarn build
	cd ..
	cp static/* $out/

check: build-ui
	cargo check
	cargo test

run: build-ui
	#!/usr/bin/env bash
	export WEBAUTHN_TINY_LOG="debug"
	state_directory="{{justfile_directory()}}/state"
	mkdir -p $state_directory
	cargo run -- --rp-id=localhost --rp-origin=http://localhost:8080 --session-secret=$(openssl rand -hex 64) --state-directory=$state_directory
