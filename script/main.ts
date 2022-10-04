// @deno-types="https://esm.sh/v96/@github/webauthn-json@2.0.1/dist/types/browser-ponyfill.d.ts"
import {
  create,
  CredentialCreationOptionsJSON,
  CredentialRequestOptionsJSON,
  get,
  parseCreationOptionsFromJSON,
  parseRequestOptionsFromJSON,
} from "https://esm.sh/@github/webauthn-json@2.0.1/browser-ponyfill.js?target=deno";

async function startAuthentication(): Promise<CredentialRequestOptions> {
  const response = await fetch("/api/authenticate", { method: "GET" });
  if (!response.ok) {
    throw new Error("Failed to start authentication");
  }
  const data: { challenge: null | CredentialRequestOptionsJSON } =
    await response.json();
  if (data.challenge === null) {
    window.alert("did not get challenge from server");
    throw new Error("did not get challenge from server");
  }
  return parseRequestOptionsFromJSON(data.challenge);
}

async function endAuthentication(opts: CredentialRequestOptions) {
  const data = await get(opts);
  const body = JSON.stringify(data);

  const response = await fetch("/api/authenticate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });

  if (!response.ok) {
    window.alert("not authenticated");
    throw new Error("not authenticated");
  }
}

document.addEventListener("DOMContentLoaded", function () {
  const deleteButton = document.getElementById("delete-button");
  if (deleteButton) {
    deleteButton.addEventListener("click", async function (ev: MouseEvent) {
      ev.preventDefault(); // TODO(jared): not needed?
      const cred_id = deleteButton.getAttribute("value");
      if (cred_id) {
        try {
          const response = await fetch(`/api/credentials/${cred_id}`, {
            method: "DELETE",
          });
          if (response.status === 204) {
            location.reload();
          }
        } catch (err) {
          console.error(err);
          window.alert(err);
        }
      }
    });
  }

  const form = document.querySelector("form");
  if (form) {
    form.addEventListener("submit", async function (ev: SubmitEvent) {
      ev.preventDefault();
      const formData = new FormData(form);
      const newCredential = formData.get("new-cred-name");
      if (newCredential === null) {
        window.alert("Name for new credential not found");
        return;
      } else if (newCredential === "") {
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

  if (location.pathname === "/authenticate") {
    startAuthentication().then(endAuthentication).catch(console.error);
  }
});
