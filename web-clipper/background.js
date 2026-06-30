// Keepr Web Clipper background worker.
//
// The worker owns all localhost writes. Popup buttons and context-menu
// entries share the same extraction + POST path so auth and payload
// limits stay centralized in the Keepr desktop server.

const api = globalThis.browser ?? globalThis.chrome;
const MENU_IDS = {
  page: "keepr-save-page",
  selection: "keepr-save-selection",
  link: "keepr-save-link",
};

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
        "Keepr endpoint not configured - open Extension Options and paste the Port + Token from Keepr Settings > Web Clipper.",
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
        "Could not reach Keepr - make sure the app is running and the port matches. (" +
        String(e?.message ?? e) +
        ")",
    };
  }
}

async function extractFromTab(tab, mode, fallback = {}) {
  if (!tab?.id) return fallbackClip(fallback);
  try {
    await api.scripting.executeScript({
      target: { tabId: tab.id },
      files: ["article-extractor.js"],
    });
  } catch (_e) {
    return fallbackClip({
      ...fallback,
      url: fallback.url || tab.url,
      title: fallback.title || tab.title || tab.url,
    });
  }
  const [{ result }] = await api.scripting.executeScript({
    target: { tabId: tab.id },
    args: [mode, fallback],
    func: (extractMode, fallbackData) => {
      return globalThis.KeeprClipperExtractor.extractReadableClip(extractMode, fallbackData);
    },
  });
  return result ?? fallbackClip(fallback);
}

function fallbackClip(fallback = {}) {
  return {
    url: fallback.url || "",
    title: fallback.title || fallback.url || "Untitled clip",
    markdown: fallback.selectionText || fallback.markdown || "",
    excerpt: fallback.excerpt || null,
  };
}

async function clipActiveTab(command, tab, fallback = {}) {
  if (command === "clip-url") {
    return postClip("/clip/url", {
      url: fallback.url || tab?.url,
      title: fallback.title || tab?.title || fallback.url || tab?.url,
      markdown: "",
      tags: ["clipped", "link"],
    });
  }
  const mode = command === "clip-selection" ? "selection" : "article";
  const data = await extractFromTab(tab, mode, fallback);
  return postClip(command === "clip-selection" ? "/clip/selection" : "/clip", {
    url: data.url || fallback.url || tab?.url,
    title: data.title || fallback.title || tab?.title || data.url || "Untitled clip",
    markdown: data.markdown,
    excerpt: data.excerpt,
    tags: command === "clip-selection" ? ["clipped", "selection"] : ["clipped", "article"],
  });
}

async function setupContextMenus() {
  if (!api.contextMenus) return;
  try {
    if (globalThis.browser?.contextMenus) {
      await api.contextMenus.removeAll();
    } else {
      await new Promise((resolve) => api.contextMenus.removeAll(resolve));
    }
  } catch (_e) {
    // Context menus are best-effort across Chrome and Firefox MV3.
  }
  api.contextMenus.create({
    id: MENU_IDS.page,
    title: "Save page to Keepr",
    contexts: ["page"],
  });
  api.contextMenus.create({
    id: MENU_IDS.selection,
    title: "Save selection to Keepr",
    contexts: ["selection"],
  });
  api.contextMenus.create({
    id: MENU_IDS.link,
    title: "Save link to Keepr",
    contexts: ["link"],
  });
}

function flashBadge(tabId, ok) {
  if (!api.action?.setBadgeText || !tabId) return;
  api.action.setBadgeText({ tabId, text: ok ? "OK" : "ERR" });
  api.action.setBadgeBackgroundColor?.({
    tabId,
    color: ok ? "#137333" : "#d93025",
  });
  setTimeout(() => api.action.setBadgeText({ tabId, text: "" }), 1600);
}

api.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (!msg || typeof msg.command !== "string") return false;
  (async () => {
    try {
      if (msg.command === "ping") {
        const cfg = await readConfig();
        if (!cfg) {
          sendResponse({ ok: false, error: "not_configured" });
          return;
        }
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
        sendResponse(await clipActiveTab("clip-url", tab));
        return;
      }
      if (msg.command === "clip-selection" || msg.command === "clip-page") {
        sendResponse(await clipActiveTab(msg.command, tab));
        return;
      }
      sendResponse({ ok: false, error: `unknown command: ${msg.command}` });
    } catch (e) {
      sendResponse({ ok: false, error: String(e?.message ?? e) });
    }
  })();
  return true;
});

api.runtime.onInstalled?.addListener(() => {
  setupContextMenus();
});

api.runtime.onStartup?.addListener(() => {
  setupContextMenus();
});

api.contextMenus?.onClicked?.addListener((info, tab) => {
  (async () => {
    let result;
    if (info.menuItemId === MENU_IDS.link) {
      result = await clipActiveTab("clip-url", tab, {
        url: info.linkUrl,
        title: info.linkText || info.linkUrl,
      });
    } else if (info.menuItemId === MENU_IDS.selection) {
      result = await clipActiveTab("clip-selection", tab, {
        selectionText: info.selectionText,
        url: tab?.url,
        title: tab?.title,
      });
    } else if (info.menuItemId === MENU_IDS.page) {
      result = await clipActiveTab("clip-page", tab, {
        url: tab?.url,
        title: tab?.title,
      });
    } else {
      return;
    }
    flashBadge(tab?.id, result.ok);
  })().catch(() => flashBadge(tab?.id, false));
});
