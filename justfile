export WEBAUTHN_TINY_LOG := "debug"

help:
	just --list

# remove nix derivations and cargo outputs
clean:
	rm -rf result* {{justfile_directory()}}/state
	cargo clean

# update README with usage string from cli's `--help` output
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

build:
	cargo build

update: update_usage
	cargo update

run port="8080":
	#!/usr/bin/env bash
	export STATE_DIRECTORY={{justfile_directory()}}/state
	mkdir -p $STATE_DIRECTORY
	password_file=$STATE_DIRECTORY/passwords
	[[ -f $password_file ]] || echo user:$(printf "password" | argon2 $(openssl rand -hex 16) -id -e) > $password_file
	session_secret_file=$STATE_DIRECTORY/session_secret
	[[ -f $session_secret_file ]] || openssl rand -hex 64 > $session_secret_file
	cargo watch --exec "run -- --address=[::]:{{port}} --rp-id=localhost --rp-origin=http://localhost:{{port}} --password-file=$password_file --session-secret-file=$session_secret_file"
