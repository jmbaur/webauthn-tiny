{ nixosTest, package, ... }:
nixosTest {
  name = "webauthn-tiny";
  nodes.machine = {
    imports = [ ./module.nix ];
    config.services.webauthn-tiny = {
      enable = true;
      inherit package;
      basicAuth = { user = "password"; };
      relyingParty.id = "foo_rp.com";
      relyingParty.origin = "https://foo_rp.com";
    };
  };
  testScript = builtins.readFile ./test.py;
}
