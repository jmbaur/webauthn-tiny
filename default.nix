{ rustPlatform
, pkg-config
, openssl
, systemd
, clippy
, ...
}:
rustPlatform.buildRustPackage {
  pname = "webauthn-tiny";
  version = "0.1.0";
  src = ./.;
  PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig:${systemd.dev}/lib/pkgconfig";
  nativeBuildInputs = [ clippy pkg-config ];
  checkPhase = ''
    cargo clippy
  '';
  cargoLock.lockFile = ./Cargo.lock;
}
