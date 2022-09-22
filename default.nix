{ rustPlatform
, pkg-config
, openssl
, sqlite
, systemd
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
  PKG_CONFIG_PATH = lib.concatMapStringsSep ":" (drv: "${drv}/lib/pkgconfig") [
    sqlite.dev
    openssl.dev
    systemd.dev
  ];
  nativeBuildInputs = [ pkg-config ];
}
