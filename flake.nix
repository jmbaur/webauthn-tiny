{
  description = "webauthn-tiny";
  inputs = {
    deno2nix.inputs.nixpkgs.follows = "nixpkgs";
    deno2nix.url = "github:SnO2WMaN/deno2nix";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
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
      packages.default = pkgs.callPackage ./. { };
      packages.ui = pkgs.symlinkJoin {
        name = "webauthn-tiny-ui";
        paths = [ ./static (pkgs.callPackage ./script { }) ];
      };
      devShells.default = pkgs.mkShell {
        inherit (preCommitHooks) shellHook;
        inherit (self.packages.${system}.default) RUSTFLAGS;
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
