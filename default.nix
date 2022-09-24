{ rustPlatform
, pkg-config
, openssl
, sqlite
, lib
, web-ui
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
  buildInputs = [ sqlite openssl ];
  nativeBuildInputs = [ pkg-config ];
  passthru = { inherit web-ui; };
}
