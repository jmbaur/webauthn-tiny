{ mkYarnPackage, just, esbuild, ... }:
mkYarnPackage {
  src = ./.;
  extraBuildInputs = [ just esbuild ];
  buildPhase = "yarn build";
  installPhase = "true";
  doDist = false;
}
