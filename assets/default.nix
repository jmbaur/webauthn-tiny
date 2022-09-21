{ mkYarnPackage
, ...
}:
mkYarnPackage {
  src = ./.;
  yarnLock = ./yarn.lock;
  packageJSON = ./package.json;
  buildPhase = ''
    yarn build
  '';
  installPhase = ''
    cp -r deps/assets/dist $out
  '';
  doDist = false;
}
