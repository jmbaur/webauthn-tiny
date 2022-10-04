{
  description = "webauthn-tiny";
  inputs = {
    deno2nix.inputs.nixpkgs.follows = "nixpkgs";
    deno2nix.url = "github:SnO2WMaN/deno2nix";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    pre-commit.inputs.nixpkgs.follows = "nixpkgs";
    pre-commit.url = "github:cachix/pre-commit-hooks.nix";
    esbuild-nixpkgs-pin.url = "nixpkgs/a2b3b7593440cbd1726bb0ec4616347652b2adb5";
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
      ESBUILD_BINARY_PATH = "${esbuild-nixpkgs-pin.legacyPackages.${system}.esbuild}/bin/esbuild";
    in
    {
      packages.nixos-test = pkgs.callPackage ./test.nix { inherit inputs; };
      packages.default = pkgs.callPackage ./. { };
      packages.ui = pkgs.callPackage ./frontend {
        inherit (self.packages.${system}) packup;
      };
      packages.packup = pkgs.callPackage ./frontend/packup {
        inherit ESBUILD_BINARY_PATH;
      };
      devShells.default = pkgs.mkShell {
        inherit (preCommitHooks) shellHook;
        inherit (self.packages.${system}.default) RUSTFLAGS;
        inherit ESBUILD_BINARY_PATH;
        WEBAUTHN_TINY_LOG = "debug";
        nativeBuildInputs = self.packages.${system}.default.nativeBuildInputs;
        buildInputs = self.packages.${system}.default.buildInputs
          ++ self.packages.${system}.ui.buildInputs
          ++ [
          (pkgs.writeShellScriptBin "update-domain-list" ''
            ${pkgs.curl}/bin/curl --silent https://www.iana.org/domains/root/db |
              ${pkgs.htmlq}/bin/htmlq --text td span a |
                ${pkgs.jq}/bin/jq -nR [inputs] > frontend/domains.json
          '')
        ];
      };
    });
}
