{ nixosTest, inputs, ... }:
nixosTest {
  name = "webauthn-tiny";
  nodes.machine = {
    imports = [ inputs.self.nixosModules.default ];
    config.services.webauthn-tiny = {
      enable = true;
      basicAuth = { user = "password"; };
      relyingParty.id = "foo_rp.com";
      relyingParty.origin = "https://foo_rp.com";
    };
  };
  testScript = builtins.readFile ./test.py;
}
