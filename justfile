build-ui:
	mkdir -p $out
	cd script && yarn build
	cp static/* $out/

build: build-ui
	cargo build

run: build-ui
	cargo run -- --rp-id localhost --rp-origin http://localhost:8080 --session-secret=$(openssl rand -hex 64)
