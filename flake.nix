{
  description = "webauthn-tiny";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    pre-commit.url = "github:cachix/pre-commit-hooks.nix";
    pre-commit.inputs.nixpkgs.follows = "nixpkgs";
  };
  outputs = inputs: with inputs; {
    overlays.default = _: prev: {
      webauthn-tiny = prev.callPackage ./. { };
    };
  } // flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ self.overlays.default ];
      };
      pre-commit-hooks = pre-commit.lib.${system}.run {
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
      packages.default = pkgs.webauthn-tiny;
      devShells.default = pkgs.mkShell {
        buildInputs = [ pkgs.clippy ];
        inherit (pre-commit-hooks) shellHook;
        inherit (pkgs.webauthn-tiny)
          PKG_CONFIG_PATH
          nativeBuildInputs;
      };
    });
}
