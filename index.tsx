import React from "react";
import { createRoot } from "react-dom/client";
import {
  create,
  CredentialCreationOptionsJSON,
  CredentialRequestOptionsJSON,
  get,
  parseCreationOptionsFromJSON,
  parseRequestOptionsFromJSON,
} from "@github/webauthn-json/browser-ponyfill";

type Credential = {
  id: string;
  name: string;
};

async function checkIfAuthenticated(): Promise<boolean> {
  const response = await fetch("/api/validate", { method: "GET" });
  return response.status === 200;
}

async function startAuthentication(): Promise<
  CredentialRequestOptions | undefined
> {
  const response = await fetch("/api/authenticate", { method: "GET" });
  const data: { challenge: null | CredentialRequestOptionsJSON } =
    await response.json();
  if (data.challenge === null) return undefined;
  return parseRequestOptionsFromJSON(data.challenge);
}

async function endAuthentication(opts: CredentialRequestOptions) {
  const data = await get(opts);
  const body = JSON.stringify(data);

  await fetch("/api/authenticate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });
}

async function getCredentials(): Promise<Array<Credential>> {
  const response = await fetch("/api/credentials", { method: "GET" });
  const data: { data: Array<Credential> } = await response.json();
  return data.data;
}

function App() {
  const [loading, setLoading] = React.useState<boolean>(false);
  const [authenticated, setAuthenticated] = React.useState<boolean>(false);
  const [refresh, setRefresh] = React.useState<boolean>(true);
  const [newCredential, setNewCredential] = React.useState<string>("");
  const [credentials, setCredentials] = React.useState<Array<Credential>>([]);

  React.useEffect(() => {
    if (!window.PublicKeyCredential) return;

    setLoading(true);
    checkIfAuthenticated().then((isAuthenticated) => {
      if (isAuthenticated) {
        setAuthenticated(true);
        setLoading(false);
      } else {
        startAuthentication().then((data) => {
          if (data !== undefined) {
            // we have a challenge
            endAuthentication(data).then(() => {
              setAuthenticated(true);
              setLoading(false);
            });
          }
        });
      }
    });
  }, [authenticated]);

  React.useEffect(() => {
    if (!authenticated) return;
    if (!refresh) return;
    getCredentials().then(setCredentials);
    setRefresh(false);
  }, [authenticated, refresh]);

  const registerCredential: React.FormEventHandler = async function (e) {
    e.preventDefault();
    if (newCredential === "") {
      alert("Name for new credential is empty");
      return;
    }
    try {
      const response = await fetch("/api/register", { method: "GET" });
      const startData: CredentialCreationOptionsJSON = await response.json();
      const endData = await create(parseCreationOptionsFromJSON(startData));
      const body = JSON.stringify({ name: newCredential, credential: endData });
      await fetch("/api/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });
      setRefresh(true);
      setNewCredential("");
    } catch (err) {
      console.error(err);
      alert(err);
    }
  };

  const deleteCredential = async function (cred_name: string) {
    try {
      await fetch(`/api/credentials/${cred_name}`, { method: "DELETE" });
      setRefresh(true);
    } catch (err) {
      console.error(err);
      alert(err);
    }
  };

  return (
    <React.Fragment>
      {loading ? <React.Fragment></React.Fragment> : (
        <React.Fragment>
          <h2>WebauthnTiny</h2>
          {authenticated
            ? (
              <React.Fragment>
                {window.PublicKeyCredential
                  ? (
                    <React.Fragment>
                      <div>
                        <h4>add a new credential</h4>
                        <form onSubmit={registerCredential}>
                          <label>
                            <input
                              type="text"
                              placeholder="name"
                              value={newCredential}
                              onChange={(e) => setNewCredential(e.target.value)}
                            />
                          </label>
                          <input type="submit" value={"\u{002b}"} />
                        </form>
                      </div>
                      <div>
                        <h4>
                          {credentials.length > 0
                            ? (
                              <React.Fragment>
                                existing credentials
                              </React.Fragment>
                            )
                            : (
                              <React.Fragment>
                                no existing credentials
                              </React.Fragment>
                            )}
                        </h4>
                        {credentials.map((cred) => (
                          <div key={cred.id}>
                            {cred.name}
                            <button onClick={() => deleteCredential(cred.name)}>
                              {"\u{2212}"}
                            </button>
                          </div>
                        ))}
                      </div>
                    </React.Fragment>
                  )
                  : <h4>this browser does not support webauthn</h4>}
              </React.Fragment>
            )
            : <h4>you are not authenticated</h4>}
        </React.Fragment>
      )}
    </React.Fragment>
  );
}

const container = document.getElementById("app");
const root = createRoot(container!);
root.render(<App />);
