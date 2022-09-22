import React from "react";
import { createRoot } from "react-dom/client";
import {
  create,
  // get,
  // parseCreationOptionsFromJSON,
  // parseRequestOptionsFromJSON,
} from "@github/webauthn-json/browser-ponyfill";

type Credential = {
  id: string;
  name: string;
};

function App() {
  const [newCredential, setNewCredential] = React.useState<string>("");
  const [credentials, setCredentials] = React.useState<Array<Credential>>([]);

  React.useEffect(() => {
    setCredentials([
      { id: "foo", name: "bar" },
      { id: "bar", name: "baz" },
    ]);
  }, []);

  // async function startAuthentication(): Promise<InProgressAuthentication> {
  //   const response = await fetch("/authenticate/start");
  //   const data = await response.json();
  //   return {
  //     username: data.username,
  //     opts: parseRequestOptionsFromJSON(data.challenge_response),
  //   };
  // }

  // async function endAuthentication(inProgress: InProgressAuthentication) {
  //   const data = await get(inProgress.opts);
  //   const body = JSON.stringify(data);
  //
  //   await fetch(`/authenticate/end/${inProgress.username}`, {
  //     method: "POST",
  //     headers: { "Content-Type": "application/json" },
  //     body,
  //   });
  // }

  if (!window.PublicKeyCredential) {
    alert("Error: this browser does not support WebAuthn");
    return;
  }

  const registerCredential: React.FormEventHandler = async function (e) {
    e.preventDefault();
    try {
      const response = await fetch("/register/start");
      const startData = await response.json();
      const endData = await create(startData.opts);
      const body = JSON.stringify(endData);
      await fetch(`/register/end/${startData.username}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });
    } catch (err) {
      alert(err);
    }
  };

  return (
    <>
      {!window.PublicKeyCredential
        ? <div>Error: this browser does not support WebAuthn</div>
        : (
          <div>
            {credentials.map((cred) => (
              <div key={cred.id}>
                {cred.name}
                <button>{"\u{2212}"}</button>
              </div>
            ))}
            <div>
              <form onSubmit={registerCredential}>
                <label>
                  <input
                    type="text"
                    placeholder="credential name"
                    value={newCredential}
                    onChange={(e) => setNewCredential(e.target.value)}
                  />
                </label>
                <input type="submit" value={"\u{002b}"} />
              </form>
            </div>
          </div>
        )}
    </>
  );
}

const container = document.getElementById("app");
const root = createRoot(container!);
root.render(<App />);
