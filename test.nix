{ nixosTest, inputs, runCommand, openssl, ... }:
nixosTest {
  name = "webauthn-tiny";
  nodes.machine = { ... }: {
    imports = [ inputs.self.nixosModules.default ];
    config.services.webauthn-tiny = {
      enable = true;
      environmentFile = runCommand "env_file" { } ''
        echo SESSION_SECRET="$(${openssl}/bin/openssl rand -hex 64)" > $out
      '';
      basicAuth = { user = "password"; };
      relyingParty.id = "foo_rp.com";
      relyingParty.origin = "https://foo_rp.com";
    };
  };
  testScript = ''
    machine.wait_for_unit("webauthn-tiny.service")
    machine.wait_for_open_port(8080)
    machine.fail("curl -v --fail [::1]:8080/authenticate")
    machine.fail("curl -v --fail -u user:wrong_password [::1]:8080/authenticate")
    machine.succeed("curl -v --fail -u user:password [::1]:8080/authenticate")
  '';
}
