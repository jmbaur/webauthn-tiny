# WebAuthnTiny

The goal of this project is to provide a mechanism for securely accessing
private resources over the internet in the simplest possible manner.

```console
Usage: webauthn-tiny [OPTIONS] --rp-id <RP_ID> --rp-origin <RP_ORIGIN> --session-secret-file <SESSION_SECRET_FILE> --password-file <PASSWORD_FILE>

Options:
      --address <ADDRESS>
          Address to bind on [env: ADDRESS=] [default: [::]:8080]
      --rp-id <RP_ID>
          Relying Party ID [env: RP_ID=]
      --rp-origin <RP_ORIGIN>
          Relying Party origin [env: RP_ORIGIN=]
      --extra-allowed-origin <EXTRA_ALLOWED_ORIGIN>
          Extra allowed origin [env: EXTRA_ALLOWED_ORIGIN=]
      --session-secret-file <SESSION_SECRET_FILE>
          Session secret file [env: SESSION_SECRET_FILE=]
      --password-file <PASSWORD_FILE>
          Password file [env: PASSWORD_FILE=]
      --state-directory <STATE_DIRECTORY>
          Directory to store program state [env: STATE_DIRECTORY=] [default: /var/lib/webauthn-tiny]
  -h, --help
          Print help information
  -V, --version
          Print version information
```

## Password File

The password file is similar to the htpasswd file format. Each username/hash
pair is on a separate line. The pair is separated by a colon, where the password
hash is an argon2 hash. An individual line in the file with a valid hash can be
generated like so:

```bash
echo username:$(systemd-ask-password -n | argon2 $(openssl rand -hex 16) -id -e)
```

## Reverse Proxy Setup

### Nginx

See [module.nix](module.nix) for an example nginx configuration.
