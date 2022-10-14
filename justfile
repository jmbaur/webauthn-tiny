# vim: ft=make

build-ui:
	deno task \
		--config {{justfile_directory()}}/script/deno.jsonc \
		--cwd {{justfile_directory()}}/script \
		build
	cp static/* $out/

build:
	cargo build
