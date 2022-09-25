{ nixosTest
, inputs
, writeText
, ...
}:

nixosTest {
  name = "webauthn-tiny";
  nodes.machine = { lib, ... }: {
    imports = [ inputs.self.nixosModules.default ];
    config.services.webauthn-tiny = {
      enable = true;
      domain = "foo_rp.com";
      basicAuth.foo = "bar";
      userFile = toString (writeText "userFile" "");
      credentialFile = toString (writeText "userFile" "");
      relyingParty.id = "foo_rp.com";
      relyingParty.origin = "https://foo_rp.com";
    };
  };
  testScript = ''
    machine.wait_for_unit("webauthn-tiny.service")
    machine.wait_for_open_port(8080)
  '';
}
