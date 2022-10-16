import {
  create,
  get,
  parseCreationOptionsFromJSON,
  parseRequestOptionsFromJSON,
} from "@github/webauthn-json/browser-ponyfill";

async function authenticate() {
  const startResponse = await fetch("/api/authenticate", { method: "GET" });
  if (!startResponse.ok) return window.alert("failed to start authentication");
  if (startResponse.status === 204) location.reload(); // no user credentials
  const endResponse = await fetch("/api/authenticate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(
      await get(
        parseRequestOptionsFromJSON(await startResponse.json()),
      ),
    ),
  });
  if (!endResponse.ok) return window.alert("not authenticated");
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
        const response = await fetch(`/api/credentials/${cred_id}`, {
          method: "DELETE",
        });
        if (!response.ok) return window.alert("failed to delete credential");
        if (response.status === 204) location.reload();
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
        return window.alert("Name for new credential is empty");
      }
      const startResponse = await fetch("/api/register", { method: "GET" });
      if (!startResponse.ok) {
        return window.alert("failed to start credential registration");
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
        window.alert("failed to end credential registration");
      } else location.reload();
    });
  }

  if (document.getElementById("authenticating-msg") !== null) {
    authenticate().catch(console.error);
  }
});
