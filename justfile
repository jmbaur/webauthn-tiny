# vim: ft=make

build-ui:
	mkdir -p $out
	cd script && yarn build
	cp static/* $out/

build:
	cargo build
