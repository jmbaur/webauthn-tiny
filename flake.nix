{
  description = "webauthn-server";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = inputs: with inputs; {
    overlays.default = _: prev: {
      webauthn-server = prev.callPackage ./. { };
    };
  } // flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system; overlays = [ self.overlays.default ];
      };
    in
    {
      packages.default = pkgs.webauthn-server;
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          libargon2
          openssl
          (writeShellScriptBin "gendata" ''
            mkdir -p data
            openssl rand -hex 12 > data/salt
            pwdhash=$(echo bar | ${libargon2}/bin/argon2 $(cat data/salt) -e)
            cat > data/passwords.yaml <<EOF
            foo: $pwdhash
            EOF
          '')
        ];
        inherit (pkgs.webauthn-server)
          PKG_CONFIG_PATH
          nativeBuildInputs;
      };
    });
}
