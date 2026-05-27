const api = globalThis.browser ?? globalThis.chrome;
const portEl = document.getElementById("port");
const tokenEl = document.getElementById("token");
const saveBtn = document.getElementById("save");
const testBtn = document.getElementById("test");
const statusEl = document.getElementById("status");

function setStatus(text, kind = "") {
  statusEl.textContent = text;
  statusEl.className = kind;
}

async function load() {
  const { port, token } = await api.storage.local.get(["port", "token"]);
  if (port) portEl.value = port;
  if (token) tokenEl.value = token;
}

function validate() {
  const port = (portEl.value || "").trim();
  const token = (tokenEl.value || "").trim();
  if (!/^\d+$/.test(port)) return "Port must be a number";
  const portN = Number(port);
  if (portN < 1 || portN > 65535) return "Port must be 1-65535";
  if (!/^[0-9a-fA-F]{64}$/.test(token)) return "Token must be 64 hexadecimal characters";
  return null;
}

saveBtn.addEventListener("click", async () => {
  const err = validate();
  if (err) { setStatus(err, "err"); return; }
  await api.storage.local.set({
    port: Number(portEl.value.trim()),
    token: tokenEl.value.trim().toLowerCase(),
  });
  setStatus("Saved.", "ok");
});

testBtn.addEventListener("click", async () => {
  const err = validate();
  if (err) { setStatus(err, "err"); return; }
  const port = Number(portEl.value.trim());
  const token = tokenEl.value.trim().toLowerCase();
  setStatus("Testing…");
  try {
    const h = await fetch(`http://127.0.0.1:${port}/health`);
    if (!h.ok) { setStatus(`Health check failed: HTTP ${h.status}`, "err"); return; }
    const body = await h.json();
    // Try a real authenticated call so we know the token is right.
    const r = await fetch(`http://127.0.0.1:${port}/clip/url`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${token}`,
      },
      // Use a marker URL the user will recognize; falls into Trash if
      // they don't want it. Better than an unauthenticated probe that
      // wouldn't actually exercise the bearer token path.
      body: JSON.stringify({
        url: "https://keepr.invalid/test-connection",
        title: "Keepr Web Clipper test (you can delete me)",
        markdown: "",
        tags: ["clipper-test"],
      }),
    });
    if (r.status === 401) { setStatus("Token rejected. Re-copy from Keepr.", "err"); return; }
    if (!r.ok) { setStatus(`HTTP ${r.status} from /clip/url`, "err"); return; }
    setStatus(`Connected to Keepr ${body.version}. Test note saved — check your notes.`, "ok");
  } catch (e) {
    setStatus("Could not reach 127.0.0.1:" + port + " — is Keepr running?", "err");
  }
});

load();
