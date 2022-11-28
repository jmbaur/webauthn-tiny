{ mkYarnPackage, just, esbuild, ... }:
mkYarnPackage {
  src = ./.;
  extraBuildInputs = [ just esbuild ];
  buildPhase = "yarn build --outdir=$out";
  installPhase = "true";
  doDist = false;
}
