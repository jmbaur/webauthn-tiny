{ rustPlatform
, pkg-config
, openssl
, systemd
, ...
}:
rustPlatform.buildRustPackage {
  pname = "webauthn-tiny";
  version = "0.1.0";
  src = ./.;
  PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig:${systemd.dev}/lib/pkgconfig";
  nativeBuildInputs = [ pkg-config ];
  cargoLock.lockFile = ./Cargo.lock;
}
