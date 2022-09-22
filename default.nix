{ rustPlatform
, pkg-config
, openssl
, systemd
, webauthn-tiny-assets
, ...
}:
rustPlatform.buildRustPackage {
  pname = "webauthn-tiny";
  version = "0.1.0";
  src = ./.;
  ASSETS_DIR = "${webauthn-tiny-assets}";
  PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig:${systemd.dev}/lib/pkgconfig";
  nativeBuildInputs = [ pkg-config ];
  cargoLock.lockFile = ./Cargo.lock;
}
