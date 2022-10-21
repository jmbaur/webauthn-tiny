build: build-ui
	cargo build

build-ui:
	mkdir -p $out
	cd script && yarn build
	cp static/* $out/

run: build-ui
	cargo run -- --rp-id localhost --rp-origin http://localhost:8080 --session-secret=$(openssl rand -hex 64)
