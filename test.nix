{ nixosTest, inputs, writeText, ... }:
nixosTest {
  name = "webauthn-tiny";
  nodes.machine = { ... }: {
    imports = [ inputs.self.nixosModules.default ];
    config.services.webauthn-tiny = {
      enable = true;
      environmentFile = writeText "env_file" ''
        SESSION_SECRET=eb62ac7bb66cf4bcfd7a2dc3a8237073a37684270ab358efca65d80d017b5d4704bbea07180b14a78b6b165f2763bbbb74905b8b8bba06a084e036db306a8193
      '';
      relyingParty.id = "foo_rp.com";
      relyingParty.origin = "https://foo_rp.com";
    };
  };
  testScript = ''
    machine.wait_for_unit("webauthn-tiny.service")
    machine.wait_for_open_port(8080)
  '';
}
