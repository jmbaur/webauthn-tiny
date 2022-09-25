{ buildGoModule, ... }:
buildGoModule {
  name = "dev";
  src = ./.;
  vendorSha256 = "sha256-rQosLQPGZiwmZrCDWLjgl1V6/iOc6bmao/H3zprCDQw=";
}
