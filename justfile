# vim: ft=make

export ASSETS_DIRECTORY := env_var("out")
export WEBAUTHN_TINY_LOG := "debug"

help:
	@just --list

# remove nix derivations and cargo/yarn outputs
clean:
	rm -rf $out/* result*
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

update: update_usage
	cargo update
	cargo upgrade
	yarn upgrade

deps:
	yarn install

build: build-ui
	cargo build

build-ui:
	yarn build --outdir=$out

check: build-ui
	cargo check
	cargo test

ci: deps check

run: build-ui
	#!/usr/bin/env bash
	state_directory="{{justfile_directory()}}/state"
	mkdir -p $state_directory
	[[ -f $state_directory/passwd_file ]] || echo "user:$(printf "password" | argon2 saltsaltsaltsalt -id -e)" >$state_directory/passwd_file  
	cargo run -- \
		--rp-id=localhost \
		--rp-origin=http://localhost:8080 \
		--session-secret=$(openssl rand -hex 64) \
		--password-file=$state_directory/passwd_file \
		--state-directory=$state_directory
