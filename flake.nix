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
      overlays.default = (_: prev: { webauthn-tiny = prev.callPackage ./. { }; });
      nixosModules.default = {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./module.nix ];
      };
      legacyPackages = forAllSystems ({ pkgs, ... }: pkgs);
      devShells = forAllSystems ({ pkgs, system, ... }: {
        default = self.devShells.${system}.ci.overrideAttrs (old: {
          buildInputs = old.buildInputs ++ [ pkgs.cargo-watch pkgs.libargon2 ];
          inherit (pre-commit.lib.${system}.run {
            src = ./.;
            hooks = { deadnix.enable = true; nixpkgs-fmt.enable = true; rustfmt.enable = true; };
          }) shellHook;
        });
        ci = pkgs.mkShell {
          inputsFrom = [ pkgs.webauthn-tiny pkgs.webauthn-tiny.ui ];
          buildInputs = with pkgs; [ just ];
        };
      });
    };
}
