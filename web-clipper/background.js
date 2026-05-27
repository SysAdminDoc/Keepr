// Keepr Web Clipper — background service worker.
//
// MV3 in Chrome/Edge ships service workers; Firefox MV3 ships event
// pages. Both load this file at extension startup and dispose it when
// idle. Don't hold long-lived state in module scope — re-read from
// storage on each event.
//
// The `api` shim works in both because Firefox aliases `chrome.*` to
// itself when an extension declares MV3.

const api = globalThis.browser ?? globalThis.chrome;

/**
 * Read the configured Keepr endpoint (port + token) from
 * chrome.storage.local. Returns { port, token } or null if unset.
 */
async function readConfig() {
  const stored = await api.storage.local.get(["port", "token"]);
  if (!stored.port || !stored.token) return null;
  return { port: String(stored.port), token: String(stored.token) };
}

async function postClip(endpoint, payload) {
  const cfg = await readConfig();
  if (!cfg) {
    return {
      ok: false,
      error:
        "Keepr endpoint not configured — open Extension Options and paste the Port + Token from Keepr's Settings → Web Clipper.",
    };
  }
  const url = `http://127.0.0.1:${cfg.port}${endpoint}`;
  try {
    const resp = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${cfg.token}`,
      },
      body: JSON.stringify(payload),
    });
    if (!resp.ok) {
      const text = await resp.text().catch(() => "");
      return { ok: false, error: `HTTP ${resp.status}: ${text || resp.statusText}` };
    }
    const json = await resp.json().catch(() => ({}));
    return { ok: true, noteId: json.noteId ?? null };
  } catch (e) {
    return {
      ok: false,
      error:
        "Could not reach Keepr — make sure the app is running and the port matches. (" +
        String(e?.message ?? e) +
        ")",
    };
  }
}

/**
 * Inject a small in-page script that grabs the active selection, the
 * page title, the URL, and the meta description. We DON'T ship
 * Readability for v0.1 — that's a follow-up. Most clips are either
 * "save the URL" or "save the selection I just highlighted", both of
 * which work fine without Readability.
 */
async function extractFromTab(tab, mode) {
  const [{ result }] = await api.scripting.executeScript({
    target: { tabId: tab.id },
    args: [mode],
    func: (extractMode) => {
      const url = location.href;
      const title = document.title || url;
      const metaDesc =
        document
          .querySelector('meta[name="description"]')
          ?.getAttribute("content") || null;
      let selection = "";
      const sel = window.getSelection();
      if (sel && !sel.isCollapsed) {
        selection = sel.toString();
      }
      if (extractMode === "selection") {
        return {
          url,
          title,
          markdown: selection,
          excerpt: metaDesc,
        };
      }
      // "page" mode: grab the meta description + the first 4 KB of
      // visible body text as a plain-text snippet. Cheap, no parser
      // dependency. The user can install a Readability-powered v2
      // build later.
      const main =
        document.querySelector("article") ??
        document.querySelector("main") ??
        document.body;
      const text = (main?.innerText ?? "").trim();
      const snippet = text.length > 4000 ? text.slice(0, 4000) + "\n…[truncated]" : text;
      return {
        url,
        title,
        markdown: snippet,
        excerpt: metaDesc,
      };
    },
  });
  return result;
}

/**
 * Public message API for the popup. The popup sends a "command" string;
 * the worker does the extract + POST and returns the result.
 */
api.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (!msg || typeof msg.command !== "string") return false;
  (async () => {
    try {
      if (msg.command === "ping") {
        const cfg = await readConfig();
        if (!cfg) { sendResponse({ ok: false, error: "not_configured" }); return; }
        // Health check the endpoint itself so the popup knows whether
        // Keepr is running BEFORE we try to clip.
        try {
          const r = await fetch(`http://127.0.0.1:${cfg.port}/health`);
          sendResponse({ ok: r.ok, port: cfg.port });
        } catch (e) {
          sendResponse({ ok: false, error: `unreachable: ${String(e?.message ?? e)}` });
        }
        return;
      }
      const [tab] = await api.tabs.query({ active: true, currentWindow: true });
      if (!tab) {
        sendResponse({ ok: false, error: "no active tab" });
        return;
      }
      if (msg.command === "clip-url") {
        const result = await postClip("/clip/url", {
          url: tab.url,
          title: tab.title || tab.url,
          markdown: "",
          tags: ["clipped"],
        });
        sendResponse(result);
        return;
      }
      if (msg.command === "clip-selection" || msg.command === "clip-page") {
        const data = await extractFromTab(
          tab,
          msg.command === "clip-selection" ? "selection" : "page",
        );
        const result = await postClip(
          msg.command === "clip-selection" ? "/clip/selection" : "/clip",
          {
            url: data.url,
            title: data.title,
            markdown: data.markdown,
            excerpt: data.excerpt,
            tags: ["clipped"],
          },
        );
        sendResponse(result);
        return;
      }
      sendResponse({ ok: false, error: `unknown command: ${msg.command}` });
    } catch (e) {
      sendResponse({ ok: false, error: String(e?.message ?? e) });
    }
  })();
  return true; // keep the channel open for the async response
});
