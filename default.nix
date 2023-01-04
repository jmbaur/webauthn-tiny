{ lib, crane, pkgsBuildHost, stdenv, qemu, pkgconfig, openssl, sqlite, ... }:
let
  toEnvVar = s: lib.replaceStrings [ "-" ] [ "_" ] (lib.toUpper s);
  target = stdenv.hostPlatform.config;
  toolchain = pkgsBuildHost.rust-bin.stable.latest.default.override { targets = [ target ]; };
  env = {
    "CARGO_TARGET_${toEnvVar target}_LINKER" = "${stdenv.cc.targetPrefix}cc";
    "CARGO_TARGET_${toEnvVar target}_RUNNER" = "qemu-${stdenv.hostPlatform.qemuArch}";
    CARGO_BUILD_TARGET = target;
    HOST_CC = "${stdenv.cc.nativePrefix}cc";
  };
in
(crane.lib.${stdenv.buildPlatform.system}.overrideToolchain toolchain).buildPackage ({
  src = ./.;
  cargoToml = ./Cargo.toml;
  depsBuildBuild = [ qemu ];
  nativeBuildInputs = [ toolchain pkgconfig ];
  buildInputs = [ sqlite openssl ];
  ASSETS_DIRECTORY = toString (pkgsBuildHost.callPackage ./ui.nix { });
  passthru = { inherit env; };
} // env)
