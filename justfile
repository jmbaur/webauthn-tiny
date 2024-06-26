export ASSETS_DIRECTORY := env_var("out")
export WEBAUTHN_TINY_LOG := "debug"

help:
	just --list

# remove nix derivations and cargo/yarn outputs
clean:
	rm -rf $out/* result*
	rm -rf node_modules
	cargo clean

# update README with usage string from cli's `--help` output
update_usage: build
	#!/usr/bin/env bash
	set -e
	readarray -t lines <<<"$(grep -n '```' README.md | cut -d':' -f1)"
	tmpfile=$(mktemp)
	echo '```console' >>$tmpfile
	cargo run -- --help 1>>$tmpfile
	echo '```' >>$tmpfile
	sed -i "${lines[0]},${lines[1]} d" README.md
	sed -i "$(("${lines[0]}" - 1)) r $tmpfile" README.md

update: deps update_usage
	cargo update
	yarn upgrade

deps:
	yarn install

build: build-ui
	cargo build

build-ui: deps
	yarn run --offline build --outdir=$out

check: build-ui
	cargo check
	cargo test

ci: deps check

run: build-ui
	#!/usr/bin/env bash
	state_directory="{{justfile_directory()}}/state"
	mkdir -p $state_directory
	password_file=$state_directory/passwords
	[[ -f $password_file ]] || echo user:$(printf "password" | argon2 $(openssl rand -hex 16) -id -e) > $password_file
	session_secret_file=$state_directory/session_secret
	[[ -f $session_secret_file ]] || openssl rand -hex 64 > $session_secret_file
	function arg() { printf "%s " $1; }
	args=()
	args+=$(arg "--rp-id=localhost")
	args+=$(arg "--rp-origin=http://localhost:8080")
	args+=$(arg "--state-directory=$state_directory")
	args+=$(arg "--password-file=$password_file")
	args+=$(arg "--session-secret-file=$session_secret_file")
	cargo watch --exec "run -- ${args[@]}"
