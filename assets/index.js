let username = "user";
let password = "password";

let headers = new Headers();
headers.append("Authorization", "Basic " + btoa(username + ":" + password));

fetch("/register", {
  headers: headers,
}).then((data) => data.json()).then((data) => {
  const publicKey = {
    ...data.publicKey,
    user: {
      ...data.publicKey.user,
      id: Uint8Array.from(data.publicKey.user.id, (c) =>
        c.charCodeAt(0)).buffer,
    },
    challenge: Uint8Array.from(
      data.publicKey.challenge,
      (c) => c.charCodeAt(0),
    ).buffer,
  };

  navigator.credentials.create({ publicKey })
    .then((newCredentialInfo) => {
      const response = newCredentialInfo.response;
      const clientExtensionsResults = newCredentialInfo
        .getClientExtensionResults();

      fetch();
    }).catch((err) => {
      console.error("ERROR", err);
    });
});