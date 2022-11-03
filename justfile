export ASSETS_DIRECTORY := env_var("out")

help:
	@just --list

update_usage:
	#!/usr/bin/env bash
	readarray -t lines <<<"$(grep -n '```' README.md | cut -d':' -f1)"
	new_usage=$(mktemp)
	echo '```console' >>$new_usage
	cargo run -- --help 2>/dev/null >>$new_usage
	echo '```' >>$new_usage
	new_first_line="$(("${lines[0]}" - 1))"
	cat README.md | sed "${lines[0]},${lines[1]} d" | sed "$new_first_line r $new_usage" | tee README.md
	rm $new_usage

update: update_usage
	cargo update
	cargo upgrade
	yarn upgrade

setup:
	yarn install

build: build-ui
	cargo build

build-ui:
	yarn run build -- --outdir=$out

check: build-ui
	cargo check
	cargo test

ci: setup check

run: build-ui
	#!/usr/bin/env bash
	export WEBAUTHN_TINY_LOG="debug"
	state_directory="{{justfile_directory()}}/state"
	mkdir -p $state_directory
	cargo run -- --rp-id=localhost --rp-origin=http://localhost:8080 --session-secret=$(openssl rand -hex 64) --state-directory=$state_directory
