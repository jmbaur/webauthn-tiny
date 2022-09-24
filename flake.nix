{
  description = "webauthn-tiny";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    pre-commit.inputs.nixpkgs.follows = "nixpkgs";
    pre-commit.url = "github:cachix/pre-commit-hooks.nix";
  };
  outputs = inputs: with inputs; {
    overlays.default = _: prev: {
      webauthn-tiny = prev.callPackage ./. {
        web-ui = prev.mkYarnPackage {
          src = ./.;
          buildPhase = "yarn build";
          installPhase = "cp -r deps/webauthn-tiny-client/dist $out";
          doDist = false;
        };
      };
    };
    nixosModules.default = {
      nixpkgs.overlays = [ self.overlays.default ];
      imports = [ ./module.nix ];
    };
  } // flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ self.overlays.default ];
      };
      preCommitHooks = pre-commit.lib.${system}.run {
        src = ./.;
        hooks = {
          cargo-check.enable = true;
          clippy.enable = true;
          nixpkgs-fmt.enable = true;
          rustfmt.enable = true;
        };
      };
    in
    {
      packages.nixos-test = pkgs.callPackage ./test.nix { inherit inputs; };
      packages.default = pkgs.webauthn-tiny;
      devShells.default = pkgs.mkShell {
        buildInputs = pkgs.webauthn-tiny.buildInputs ++ [ pkgs.sqlite ];
        inherit (pkgs.webauthn-tiny) nativeBuildInputs;
        inherit (preCommitHooks) shellHook;
      };
    });
}
