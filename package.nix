{
  lib,
  callPackage,
  rustPlatform,
  pkg-config,
  openssl,
  sqlite,
  clippy,
}:
let
  # TODO(jared): use `finalAttrs` once buildRustPackage supports it
  # https://github.com/NixOS/nixpkgs/pull/194475
  package = rustPlatform.buildRustPackage {
    pname = "webauthn-tiny";
    version = "0.2.3";
    src = lib.fileset.toSource {
      root = ./.;
      fileset = lib.fileset.unions [
        ./Cargo.toml
        ./Cargo.lock
        ./templates
        ./src
      ];
    };
    cargoLock.lockFile = ./Cargo.lock;
    strictDeps = true;
    nativeCheckInputs = [
      clippy
    ];
    nativeBuildInputs = [ pkg-config ];
    buildInputs = [
      sqlite
      openssl
    ];
    preCheck = ''
      echo "Running clippy..."
      cargo clippy -- -Dwarnings
    '';
    passthru.tests.nixos = callPackage ./test.nix { inherit package; };
    meta.mainProgram = "webauthn-tiny";
  };
in
package
