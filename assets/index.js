function toBuffer(data) {
  return Uint8Array.from(data, (c) => c.charCodeAt(0));
}

fetch("/authenticate/start").then((data) => data.json()).then(
  ({ publicKey }) => {
    publicKey.challenge = toBuffer(publicKey.challenge);
    publicKey.allowCredentials = publicKey.allowCredentials.map((cred) => ({
      ...cred,
      id: toBuffer(cred.id),
    }));

    if (!window.PublicKeyCredential) {
      alert("Error: this browser does not support WebAuthn");
      return;
    }

    navigator.credentials.get({ publicKey }).then((data) => {
      console.table(data);
      fetch(`/authenticate/end/${username}`, { method: "POST", body: data })
        .then(console.table).catch(console.error);
    }).catch(console.error);
  },
).catch((err) => {
  console.error(err);
  fetch("/register/start").then((data) => data.json()).then(({ publicKey }) => {
    publicKey.challenge = toBuffer(publicKey.challenge);
    publicKey.user.id = toBuffer(publicKey.user.id);

    if (!window.PublicKeyCredential) {
      alert("Error: this browser does not support WebAuthn");
      return;
    }

    navigator.credentials.create({ publicKey })
      .then((data) => {
        console.table(data);
        fetch(`/register/end/${username}`, { method: "POST", body: data })
          .then(console.table).catch(console.error);
      }).catch(console.error);
  });
});
