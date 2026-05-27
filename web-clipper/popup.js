// Keepr Web Clipper popup.
//
// On open, ping the background worker to check whether Keepr is
// reachable; enable the clip buttons only if the health check
// succeeded. Each button sends a single message and reflects the
// result inline.

const api = globalThis.browser ?? globalThis.chrome;

const statusEl = document.getElementById("status");
const btnPage = document.getElementById("clip-page");
const btnSelection = document.getElementById("clip-selection");
const btnUrl = document.getElementById("clip-url");

function setStatus(text, kind = "") {
  statusEl.textContent = text;
  statusEl.className = "status" + (kind ? " " + kind : "");
}

function sendCommand(command) {
  return new Promise((resolve) => {
    api.runtime.sendMessage({ command }, (resp) => resolve(resp ?? { ok: false, error: "no response" }));
  });
}

async function ping() {
  const r = await sendCommand("ping");
  if (r.ok) {
    setStatus(`Connected to Keepr on port ${r.port}`, "ok");
    btnPage.disabled = false;
    btnSelection.disabled = false;
    btnUrl.disabled = false;
  } else if (r.error === "not_configured") {
    setStatus("Open Options and paste the port + token from Keepr → Settings → Web Clipper.", "err");
  } else {
    setStatus("Keepr is not reachable — is the app running? " + (r.error ?? ""), "err");
  }
}

async function clipAndReport(command, label) {
  setStatus(`Saving ${label}…`);
  btnPage.disabled = btnSelection.disabled = btnUrl.disabled = true;
  const r = await sendCommand(command);
  if (r.ok) {
    setStatus(`Saved to Keepr.`, "ok");
    window.setTimeout(() => window.close(), 800);
  } else {
    setStatus(r.error ?? "Failed", "err");
    btnPage.disabled = btnSelection.disabled = btnUrl.disabled = false;
  }
}

btnPage.addEventListener("click", () => clipAndReport("clip-page", "page"));
btnSelection.addEventListener("click", () => clipAndReport("clip-selection", "selection"));
btnUrl.addEventListener("click", () => clipAndReport("clip-url", "URL"));

ping();
