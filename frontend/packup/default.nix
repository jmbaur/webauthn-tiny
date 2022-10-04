{ deno2nix
, fetchFromGitHub
, deno
, writeShellScriptBin
, ESBUILD_BINARY_PATH
, ...
}:
let
  bundled = deno2nix.mkBundled rec {
    pname = "packup";
    version = "0.2.1";
    src = fetchFromGitHub {
      owner = "kt3k";
      repo = pname;
      rev = "v${version}";
      sha256 = "sha256-b04FiSh70Ez4q7VXMqvna1dgPSZ6/cP7W0CbrYH6VBk=";
    };
    lockfile = ./lock.json;
    output = "packup";
    entrypoint = "cli.ts";
  };
in
writeShellScriptBin "packup" ''
  export ESBUILD_BINARY_PATH=${ESBUILD_BINARY_PATH}
  exec -a "$0" ${deno}/bin/deno run -A ${bundled}/dist/packup "$@"
''
