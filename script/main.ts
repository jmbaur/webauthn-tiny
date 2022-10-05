// @deno-types="https://esm.sh/v96/@github/webauthn-json@2.0.1/dist/types/browser-ponyfill.d.ts"
import {
  create,
  CredentialCreationOptionsJSON,
  CredentialRequestOptionsJSON,
  get,
  parseCreationOptionsFromJSON,
  parseRequestOptionsFromJSON,
} from "https://esm.sh/@github/webauthn-json@2.0.1/browser-ponyfill.js?target=deno";

async function validateLoggedIn(): Promise<boolean> {
  const response = await fetch("/api/validate");
  return response.status === 200;
}

async function startAuthentication(): Promise<CredentialRequestOptions> {
  const response = await fetch("/api/authenticate", { method: "GET" });
  if (!response.ok) {
    const msg = "failed to start authentication";
    window.alert(msg);
    throw new Error(msg);
  }
  if (response.status === 204) location.reload(); // no user credentials
  const challenge: CredentialRequestOptionsJSON = await response.json();
  return parseRequestOptionsFromJSON(challenge);
}

async function endAuthentication(opts: CredentialRequestOptions) {
  const data = await get(opts);
  const body = JSON.stringify(data);

  const response = await fetch("/api/authenticate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });

  if (!response.ok) return window.alert("not authenticated");
  location.replace("/authenticate"); // client is now logged in
}

document.addEventListener("DOMContentLoaded", function () {
  const deleteButtons = document.getElementsByClassName("delete-credential");
  for (const button of deleteButtons) {
    button.addEventListener("click", async function (_: Event) {
      const cred_id = button.getAttribute("value");
      if (
        cred_id &&
        window.confirm("Do you really want to delete this credential?")
      ) {
        try {
          const response = await fetch(`/api/credentials/${cred_id}`, {
            method: "DELETE",
          });
          if (response.status === 204) location.reload();
        } catch (err) {
          console.error(err);
          window.alert(err);
        }
      }
    });
  }

  const addButton = document.getElementById("add-credential");
  if (addButton) {
    addButton.addEventListener("click", async function (_: Event) {
      const newCredential = window.prompt(
        "Enter a name for the new credential",
      );
      if (newCredential === null) return;
      if (newCredential === "") {
        window.alert("Name for new credential is empty");
        return;
      }
      try {
        const response = await fetch("/api/register", { method: "GET" });
        const startData: CredentialCreationOptionsJSON = await response.json();
        const endData = await create(parseCreationOptionsFromJSON(startData));
        const body = JSON.stringify({
          name: newCredential,
          credential: endData,
        });
        await fetch("/api/register", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body,
        });
        location.reload();
      } catch (err) {
        console.error(err);
        window.alert(err);
      }
    });
  }

  if (document.getElementById("authenticating-msg") !== null) {
    startAuthentication().then(endAuthentication).catch(console.error);
  }
});
