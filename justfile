# vim: ft=make

export ASSETS_DIRECTORY := env_var("out")

help:
	@just --list

update_usage:
	#!/usr/bin/env bash
	set -e
	readarray -t lines <<<"$(grep -n '```' README.md | cut -d':' -f1)"
	tmpfile=$(mktemp)
	echo '```console' >>$tmpfile
	cargo run -- --help 1>>$tmpfile
	echo '```' >>$tmpfile
	sed -i "${lines[0]},${lines[1]} d" README.md
	sed -i "$(("${lines[0]}" - 1)) r $tmpfile" README.md

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
	cargo run -- \
		--rp-id=localhost \
		--rp-origin=http://localhost:8080 \
		--session-secret=$(openssl rand -hex 64) \
		--state-directory=$state_directory
