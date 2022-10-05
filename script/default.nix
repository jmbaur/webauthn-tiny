{ stdenv, deno2nix, deno, jq, lib, ... }:
let
  cargoTOML = lib.importTOML ../Cargo.toml;
  deps = deno2nix.internal.mkDepsLink ./deno.lock;
in
stdenv.mkDerivation {
  pname = cargoTOML.package.name + "ui";
  inherit (cargoTOML.package) version;
  src = ./.;
  buildInputs = [ deno jq ];
  configurePhase = ''
    export HOME=/tmp
    ln -s "${deps}" $(deno info --json | jq -r .modulesCache)
  '';
  buildPhase = "deno task build";
  checkPhase = "deno task check";
  installPhase = "cp -r dist $out";
}
