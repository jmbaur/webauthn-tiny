{
  description = "A tiny webauthn server";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    pre-commit.inputs.nixpkgs.follows = "nixpkgs";
    pre-commit.url = "github:cachix/pre-commit-hooks.nix";
  };
  outputs = inputs: with inputs;
    let
      forAllSystems = cb: nixpkgs.lib.genAttrs [ "aarch64-linux" "x86_64-linux" "aarch64-darwin" "x86_64-darwin" ] (system: cb {
        inherit system;
        pkgs = import nixpkgs { inherit system; overlays = [ self.overlays.default ]; };
      });
    in
    {
      overlays.default = _: prev: {
        webauthn-tiny = prev.callPackage ./. { ui = prev.buildPackages.callPackage ./ui.nix { }; };
      };
      nixosModules.default = {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./module.nix ];
      };
      devShells = forAllSystems ({ pkgs, system, ... }: {
        default = self.devShells.${system}.ci.overrideAttrs (old: {
          buildInputs = old.buildInputs ++ [ pkgs.cargo-watch pkgs.libargon2 ];
          inherit (pre-commit.lib.${system}.run {
            src = ./.;
            hooks = { clippy.enable = true; deadnix.enable = true; nixpkgs-fmt.enable = true; rustfmt.enable = true; };
          }) shellHook;
        });
        ci = pkgs.mkShell {
          inputsFrom = [ pkgs.webauthn-tiny ];
          inherit (pkgs.webauthn-tiny) RUSTFLAGS;
          buildInputs = with pkgs; [ cargo-edit just yarn nodejs esbuild ];
        };
      });
      packages = forAllSystems ({ pkgs, ... }: {
        nixos-test = pkgs.callPackage ./test.nix { inherit inputs; };
        default = pkgs.webauthn-tiny;
      });
    };
}
