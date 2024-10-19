{
  lib,
  callPackage,
  rustPlatform,
  pkg-config,
  openssl,
  sqlite,
}:
let
  # TODO(jared): use `finalAttrs` once buildRustPackage supports it
  # https://github.com/NixOS/nixpkgs/pull/194475
  package = rustPlatform.buildRustPackage {
    pname = "webauthn-tiny";
    version = "0.1.0";
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
    nativeBuildInputs = [ pkg-config ];
    buildInputs = [
      sqlite
      openssl
    ];
    passthru.tests.nixos = callPackage ./test.nix { inherit package; };
  };
in
package
