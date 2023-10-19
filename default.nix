{ callPackage, rustPlatform, mkYarnPackage, esbuild, pkg-config, openssl, sqlite }:
let
  ui = mkYarnPackage {
    src = ./.;
    extraBuildInputs = [ esbuild ];
    buildPhase = "yarn build --outdir=$out";
    installPhase = "true";
    doDist = false;
  };
  # TODO(jared): use `finalAttrs` once buildRustPackage supports it
  # https://github.com/NixOS/nixpkgs/pull/194475
  package = rustPlatform.buildRustPackage {
    pname = "webauthn-tiny";
    version = "0.1.0";
    src = ./.;
    cargoLock.lockFile = ./Cargo.lock;
    strictDeps = true;
    nativeBuildInputs = [ pkg-config ];
    buildInputs = [ sqlite openssl ];
    env.ASSETS_DIRECTORY = toString ui;
    passthru = {
      inherit ui;
      tests.nixos = callPackage ./test.nix { inherit package; };
    };
  };
in
package
