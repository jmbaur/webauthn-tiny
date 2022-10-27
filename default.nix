{ rustPlatform, llvmPackages_latest, pkg-config, openssl, sqlite, lib, ui, ... }:
let
  cargoToml = lib.importTOML ./Cargo.toml;
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;
in
rustPlatform.buildRustPackage {
  inherit pname version;
  src = ./.;
  buildInputs = [ sqlite openssl ];
  nativeBuildInputs = [ llvmPackages_latest.bintools pkg-config ];
  ASSETS_DIRECTORY = builtins.toPath ui;
  RUSTFLAGS = "-C link-arg=-fuse-ld=lld";
  cargoLock.lockFile = ./Cargo.lock;
}
