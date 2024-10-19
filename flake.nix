{
  description = "A tiny WebAuthn server";
  inputs = {
    git-hooks.inputs.nixpkgs.follows = "nixpkgs";
    git-hooks.url = "github:cachix/git-hooks.nix";
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };
  outputs =
    {
      self,
      nixpkgs,
      git-hooks,
    }:
    {
      overlays.default = (_: prev: { webauthn-tiny = prev.callPackage ./package.nix { }; });
      nixosModules.default = {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./module.nix ];
      };
      legacyPackages =
        nixpkgs.lib.genAttrs
          [
            "aarch64-linux"
            "x86_64-linux"
          ]
          (
            system:
            import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            }
          );
      devShells = nixpkgs.lib.mapAttrs (system: pkgs: {
        default = self.devShells.${system}.ci.overrideAttrs (old: {
          buildInputs = old.buildInputs ++ [
            pkgs.cargo-watch
            pkgs.libargon2
          ];
          inherit
            (git-hooks.lib.${system}.run {
              src = ./.;
              hooks = {
                deadnix.enable = true;
                nixfmt-rfc-style.enable = true;
                rustfmt.enable = true;
              };
            })
            shellHook
            ;
        });
        ci = pkgs.mkShell {
          inputsFrom = [ pkgs.webauthn-tiny ];
          packages = [ pkgs.just ];
        };
      }) self.legacyPackages;
    };
}
