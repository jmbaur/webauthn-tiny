{ rustPlatform, pkg-config, openssl, ... }:
rustPlatform.buildRustPackage {
  pname = "webauthn-tiny";
  version = "0.1.0";
  src = ./.;
  PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig";
  nativeBuildInputs = [ pkg-config openssl ];
  cargoLock.lockFile = ./Cargo.lock;
}
