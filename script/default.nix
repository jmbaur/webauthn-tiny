{ stdenv
, deno2nix
, deno
, jq
, ...
}:
let
  deps = deno2nix.internal.mkDepsLink ./deno.lock;
  drv = stdenv.mkDerivation {
    pname = "webauthn-tiny-ui";
    version = "0.1.1";
    src = ./.;
    buildInputs = [ deno jq ];
    configurePhase = ''
      export HOME=/tmp
      ln -s "${deps}" $(deno info --json | jq -r .modulesCache)
    '';
    buildPhase = "deno task build";
    checkPhase = "deno task check";
    installPhase = "cp -r dist $out";
  };
in
drv
