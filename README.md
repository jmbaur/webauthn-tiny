# WebAuthnTiny

The goal of this project is to provide a mechanism for securely accessing
private resources over the internet in the simplest possible manner. It _only_
handles the WebAuthn side of things, so you must manage 1FA outside of this. The
server relies on an `X-Remote-User` header being set in order to determine which
user is requesting for webauthn services. It is highly recommended to use this
server with a reverse proxy that is protected by username and password. The
reverse proxy can then set the required `X-Remote-User` header before proxying a
request.

## Usage

```console
Usage: webauthn-tiny [OPTIONS] --rp-id <RP_ID> --rp-origin <RP_ORIGIN> --session-secret <SESSION_SECRET>

Options:
      --address <ADDRESS>
          Address to bind on [env: ADDRESS=] [default: [::]:8080]
      --rp-id <RP_ID>
          Relying Party ID [env: RP_ID=]
      --rp-origin <RP_ORIGIN>
          Relying Party origin [env: RP_ORIGIN=]
      --extra-allowed-origin <EXTRA_ALLOWED_ORIGIN>
          Extra allowed origin [env: EXTRA_ALLOWED_ORIGIN=]
      --session-secret <SESSION_SECRET>
          Session secret [env: SESSION_SECRET=]
      --state-directory <STATE_DIRECTORY>
          Directory to store program state [env: STATE_DIRECTORY=] [default: /var/lib/webauthn-tiny]
  -h, --help
          Print help information
  -V, --version
          Print version information
```

## Reverse Proxy Setup

### Nginx

See [module.nix](module.nix) for an example nginx configuration.
