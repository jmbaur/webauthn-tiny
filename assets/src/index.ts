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

  const response = await fetch(`/authenticate/end/${inProgress.username}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });

  console.log(response);
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

  const response = await fetch(`/register/end/${inProgress.username}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });

  console.log(response);
}

function main() {
  if (!window.PublicKeyCredential) {
    alert("Error: this browser does not support WebAuthn");
    return;
  }

  startAuthentication().then(endAuthentication).catch(
    (err) => {
      console.error(err);
      startRegistration().then(endRegistration).catch(console.error);
    },
  );
}

main();
