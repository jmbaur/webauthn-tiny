{ nixosTest, package, ... }:
nixosTest {
  name = "webauthn-tiny";
  nodes.machine = {
    imports = [ ./module.nix ];
    config.services.webauthn-tiny = {
      enable = true;
      inherit package;
      basicAuth = {
        user = "password";
      };
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
