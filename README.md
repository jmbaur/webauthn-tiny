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

```bash
webauthn-tiny --id <RP ID> --origin <RP Origin>
```

Example:

```bash
webauthn-tiny --id mywebsite.com --origin https://auth.mywebsite.com
```

## Reverse Proxy Setup

### Nginx

See [module.nix](module.nix) for an example nginx configuration.
