m = machine  # type: ignore
m.wait_for_unit("webauthn-tiny.service")
m.wait_for_open_port(8080)
m.fail("curl -v --fail [::1]:8080/authenticate")
m.fail("curl -v --fail -u user:wrong_password [::1]:8080/authenticate")
m.succeed("curl -v --fail -u user:password [::1]:8080/authenticate")
