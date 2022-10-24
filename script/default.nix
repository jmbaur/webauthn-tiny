{ mkYarnPackage, esbuild, ... }:
mkYarnPackage {
  src = ./.;
  extraBuildInputs = [ esbuild ];
  buildPhase = "yarn build";
  installPhase = "true";
  doDist = false;
}
