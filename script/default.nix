{ mkYarnPackage, esbuild, ... }:
mkYarnPackage {
  src = ./.;
  yarnNix = ./yarn.nix;
  extraBuildInputs = [ esbuild ];
  buildPhase = "yarn build";
  installPhase = "true";
  doDist = false;
}
