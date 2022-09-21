{ rustPlatform
, pkg-config
, openssl
, systemd
, clippy
, webauthn-tiny-assets
, ...
}:
rustPlatform.buildRustPackage {
  pname = "webauthn-tiny";
  version = "0.1.0";
  src = ./.;
  ASSETS_DIR = "${webauthn-tiny-assets}";
  PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig:${systemd.dev}/lib/pkgconfig";
  nativeBuildInputs = [ clippy pkg-config ];
  checkPhase = ''
    cargo clippy
  '';
  cargoLock.lockFile = ./Cargo.lock;
}
