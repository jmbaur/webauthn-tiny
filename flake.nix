{
  description = "A tiny WebAuthn server";
  inputs = {
    git-hooks.inputs.nixpkgs.follows = "nixpkgs";
    git-hooks.url = "github:cachix/git-hooks.nix";
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };
  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs.lib)
        const
        genAttrs
        mapAttrs
        ;
    in
    {
      overlays.default = _: prev: { webauthn-tiny = prev.callPackage ./package.nix { }; };
      checks = mapAttrs (const (pkgs: {
        default = pkgs.webauthn-tiny;
      })) inputs.self.legacyPackages;
      nixosModules.default = {
        nixpkgs.overlays = [ inputs.self.overlays.default ];
        imports = [ ./module.nix ];
      };
      legacyPackages =
        genAttrs
          [
            "aarch64-linux"
            "x86_64-linux"
          ]
          (
            system:
            import inputs.nixpkgs {
              inherit system;
              overlays = [ inputs.self.overlays.default ];
            }
          );
      devShells = mapAttrs (system: pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ pkgs.webauthn-tiny ];
          packages = [
            pkgs.cargo-watch
            pkgs.just
            pkgs.libargon2
          ];
          inherit
            (inputs.git-hooks.lib.${system}.run {
              src = ./.;
              hooks = {
                deadnix.enable = true;
                nixfmt.enable = true;
                rustfmt.enable = true;
                statix.enable = true;
              };
            })
            shellHook
            ;
        };
      }) inputs.self.legacyPackages;
    };
}
