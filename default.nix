{ rustPlatform, llvmPackages_latest, pkg-config, openssl, sqlite, lib, ui, ... }:
let cargoToml = lib.importTOML ./Cargo.toml; in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;
  src = ./.;
  buildInputs = [ sqlite openssl ];
  nativeBuildInputs = [ llvmPackages_latest.bintools pkg-config ];
  ASSETS_DIRECTORY = toString ui;
  RUSTFLAGS = "-C link-arg=-fuse-ld=lld";
  cargoLock.lockFile = ./Cargo.lock;
}
