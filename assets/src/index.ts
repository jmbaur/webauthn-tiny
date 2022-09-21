import {
  create,
  get,
  parseCreationOptionsFromJSON,
  parseRequestOptionsFromJSON,
} from "@github/webauthn-json/browser-ponyfill";

type InProgress<T> = {
  username: string;
  opts: T;
};
type InProgressAuthentication = InProgress<CredentialRequestOptions>;
type InProgressRegistration = InProgress<CredentialCreationOptions>;

async function startAuthentication(): Promise<InProgressAuthentication> {
  const response = await fetch("/authenticate/start");
  const data = await response.json();
  return {
    username: data.username,
    opts: parseRequestOptionsFromJSON(data.challenge_response),
  };
}

async function endAuthentication(inProgress: InProgressAuthentication) {
  const data = await get(inProgress.opts);
  const body = JSON.stringify(data);

  await fetch(`/authenticate/end/${inProgress.username}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });
}

async function startRegistration(): Promise<InProgressRegistration> {
  const response = await fetch("/register/start");
  const data = await response.json();
  return {
    username: data.username,
    opts: parseCreationOptionsFromJSON(data.challenge_response),
  };
}

async function endRegistration(inProgress: InProgressRegistration) {
  const data = await create(inProgress.opts);
  const body = JSON.stringify(data);

  await fetch(`/register/end/${inProgress.username}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });
}

async function main() {
  if (!window.PublicKeyCredential) {
    alert("Error: this browser does not support WebAuthn");
    return;
  }

  let triedAuth = 0;
  let triedReg = 0;

  for (;;) {
    if (triedAuth < 2) {
      try {
        const auth = await startAuthentication();
        await endAuthentication(auth);
        break;
      } catch (err) {
        console.error("failed to authenticate", err);
      }
      triedAuth++;
    } else break;
    if (triedReg < 1) {
      try {
        const reg = await startRegistration();
        triedReg++;
        await endRegistration(reg);
        continue;
      } catch (err) {
        console.error("failed to register", err);
        break; // if registration fails, quit
      }
    } else break;
  }
}

main();
