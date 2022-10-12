{
  description = "webauthn-tiny";
  inputs = {
    deno2nix.inputs.nixpkgs.follows = "nixpkgs";
    deno2nix.url = "github:SnO2WMaN/deno2nix";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs?rev=02902f604a39bab67cfb73ceb0182183173b5a24";
    pre-commit.inputs.nixpkgs.follows = "nixpkgs";
    pre-commit.url = "github:cachix/pre-commit-hooks.nix";
  };
  outputs = inputs: with inputs; {
    nixosModules.default = {
      nixpkgs.overlays = [
        (_: prev: {
          webauthn-tiny = self.packages.${prev.system}.default;
          webauthn-tiny-ui = self.packages.${prev.system}.ui;
        })
      ];
      imports = [ ./module.nix ];
    };
  } // flake-utils.lib.eachSystem [ "aarch64-linux" "x86_64-linux" ] (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ deno2nix.overlays.default ];
      };
      preCommitHooks = pre-commit.lib.${system}.run {
        src = ./.;
        hooks = {
          cargo-check.enable = true;
          clippy.enable = true;
          deno-fmt = { enable = true; entry = "${pkgs.deno}/bin/deno fmt"; types_or = [ "markdown" "ts" "tsx" "json" ]; };
          nixpkgs-fmt.enable = true;
          rustfmt.enable = true;
        };
      };
    in
    {
      packages.nixos-test = pkgs.callPackage ./test.nix { inherit inputs; };
      packages.default = pkgs.callPackage ./. {
        ui-assets = self.packages.${system}.ui;
      };
      packages.script = pkgs.callPackage ./script { };
      packages.ui = pkgs.symlinkJoin {
        name = "webauthn-tiny-ui";
        paths = [ ./static self.packages.${system}.script ];
      };
      devShells.default = pkgs.mkShell {
        inherit (preCommitHooks) shellHook;
        inherit (self.packages.${system}.default) RUSTFLAGS;
        WEBAUTHN_TINY_LOG = "debug";
        nativeBuildInputs = self.packages.${system}.default.nativeBuildInputs;
        buildInputs = self.packages.${system}.default.buildInputs ++ self.packages.${system}.script.buildInputs ++ [
          (pkgs.writeShellScriptBin "rebuild" ''
            set -e
            pushd script
            deno task build
            popd
            cp static/* $out/
          '')
        ];
      };
    });
}
