# WebAuthnTiny

The goal of this project is to provide a mechanism for securely accessing
private resources over the internet in the simplest possible manner. It _only_
handles the WebAuthn side of things, so you must manage 1FA outside of this.
After 1FA, the server relies on an `X-Remote-User` header being set in order to
determine which user is requesting for webauthn services. This project pairs
well with Nginx when using Nginx's builtin basic auth. This allows for Nginx to
handle the username/password side of things, then it set's the `X-Remote-User`
header for that user.
