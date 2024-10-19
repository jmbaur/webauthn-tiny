import {
  create,
  get,
  parseCreationOptionsFromJSON,
  parseRequestOptionsFromJSON,
} from "https://cdn.jsdelivr.net/npm/@github/webauthn-json@2.1.1/browser-ponyfill/+esm";
document.addEventListener("DOMContentLoaded", () => {
  for (const button of document.getElementsByClassName("delete-credential")) {
    button.addEventListener("click", async function (_) {
      const cred_id = button.getAttribute("value");
      if (cred_id && window.confirm("Do you want to delete this credential?")) {
        const response = await fetch(`/api/credentials/${cred_id}`, {
          method: "DELETE",
        });
        if (!response.ok) return window.alert("Failed to delete credential");
        else if (response.status === 204) return location.reload();
      }
    });
  }
  const addButton = document.getElementById("add-credential");
  if (addButton != null) {
    addButton.addEventListener("click", async function (_) {
      const newCredential = window.prompt("Enter name for the new credential");
      if (newCredential === null) return;
      else if (newCredential === "") {
        return window.alert("Name for new credential is empty");
      }
      const startResponse = await fetch("/api/register", { method: "GET" });
      if (!startResponse.ok) {
        return window.alert("Failed to start credential registration");
      }
      const endResponse = await fetch("/api/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: newCredential,
          credential: await create(
            parseCreationOptionsFromJSON(await startResponse.json()),
          ),
        }),
      });
      if (!endResponse.ok) {
        window.alert("Failed to end credential registration");
      } else location.reload();
    });
  }
  if (document.getElementById("authenticating-msg") !== null) {
    (async () => {
      const startResponse = await fetch("/api/authenticate", { method: "GET" });
      if (!startResponse.ok) {
        return window.alert("Failed to start authentication");
      } else if (startResponse.status === 204) return location.reload(); // no user credentials
      const endResponse = await fetch("/api/authenticate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(
          await get(parseRequestOptionsFromJSON(await startResponse.json())),
        ),
      });
      if (!endResponse.ok) return window.alert("Not authenticated");
      return location.replace("/authenticate"); // client is now logged in
    })().catch(console.error);
  }
});
