{
  description = "webauthn-tiny";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    godev.inputs.nixpkgs.follows = "nixpkgs";
    godev.url = "github:jmbaur/godev";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    pre-commit.inputs.nixpkgs.follows = "nixpkgs";
    pre-commit.url = "github:cachix/pre-commit-hooks.nix";
  };
  outputs = inputs: with inputs; {
    overlays.default = _: prev: {
      webauthn-tiny-ui = prev.mkYarnPackage {
        src = ./frontend;
        extraBuildInputs = [ inputs.godev.packages.${prev.system}.default ];
        checkPhase = "yarn run check";
        buildPhase = "yarn run build";
        installPhase = "cp -r deps/webauthn-tiny-ui/dist $out";
        doDist = false;
      };
      webauthn-tiny = prev.callPackage ./. { };
    };
    nixosModules.default = {
      nixpkgs.overlays = [ self.overlays.default ];
      imports = [ ./module.nix ];
    };
  } // flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ godev.overlays.default self.overlays.default ];
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
      packages.ui = pkgs.webauthn-tiny-ui;
      packages.default = pkgs.webauthn-tiny;
      devShells.default = pkgs.mkShell {
        inherit (preCommitHooks) shellHook;
        inherit (pkgs.webauthn-tiny) RUSTFLAGS;
        WEBAUTHN_TINY_LOG = "debug";
        nativeBuildInputs = pkgs.webauthn-tiny.nativeBuildInputs;
        buildInputs = pkgs.webauthn-tiny.buildInputs
          ++ pkgs.webauthn-tiny-ui.buildInputs
          ++ [
          pkgs.cargo-edit
          pkgs.godev
          (pkgs.writeShellScriptBin "update-domain-list" ''
            ${pkgs.curl}/bin/curl --silent https://www.iana.org/domains/root/db |
              ${pkgs.htmlq}/bin/htmlq --text td span a |
                ${pkgs.jq}/bin/jq -nR [inputs] > frontend/domains.json
          '')
        ];
      };
    });
}
