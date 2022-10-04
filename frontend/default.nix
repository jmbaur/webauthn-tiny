{ stdenv
, deno2nix
, deno
, jq
, packup
, ...
}:
let
  deps = deno2nix.internal.mkDepsLink ./lock.json;
  drv = stdenv.mkDerivation {
    pname = "webauthn-tiny-ui";
    version = "0.1.1";
    src = ./.;
    buildInputs = [ packup deno jq ];
    configurePhase = ''
      export HOME=/tmp
      ln -s "${deps}" $(deno info --json | jq -r .modulesCache)
    '';
    buildPhase = ''
      deno task build
    '';
    checkPhase = ''
      deno task test
    '';
    installPhase = ''
      cp -r dist $out
    '';
    passthru = { inherit deps; };
  };
in
drv
