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
      webauthn-tiny-assets = prev.callPackage ./assets { };
    };
    nixosModules.default = import ./module.nix inputs;
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
      packages.nixos-test = pkgs.callPackage ./test.nix { inherit inputs; };
      packages.webauthn-tiny = pkgs.webauthn-tiny;
      packages.webauthn-tiny-assets = pkgs.webauthn-tiny-assets;
      devShells.web = pkgs.mkShell {
        inherit (pkgs.webauthn-tiny-assets) nativeBuildInputs buildInputs;
      };
      devShells.default = pkgs.mkShell {
        ASSETS_DIR = "assets/dist";
        buildInputs = with pkgs; [ clippy cargo-watch ];
        inherit (pkgs.webauthn-tiny) PKG_CONFIG_PATH nativeBuildInputs;
        inherit (pre-commit-hooks) shellHook;
      };
    });
}
