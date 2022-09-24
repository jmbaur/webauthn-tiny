{ rustPlatform
, pkg-config
, openssl
, sqlite
, assets
, lib
, ...
}:
let
  cargoToml = lib.importTOML ./Cargo.toml;
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;
in
rustPlatform.buildRustPackage {
  inherit pname version;
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
  ASSETS_PATH = "${assets}";
  buildInputs = [ sqlite openssl ];
  nativeBuildInputs = [ pkg-config ];
}
