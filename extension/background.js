// zman browser extension: reports the active tab's URL and title to the zman
// tracker running on this computer (127.0.0.1). It never talks to any other
// host, and it never reports private/incognito tabs.
//
// zman answers whether the tab must be blocked (focus session, daily limit,
// schedule or blocked site); if so the tab is sent to the "blocked" page.

const api = globalThis.browser ?? globalThis.chrome;
const DEFAULT_PORT = 27184;

async function endpoint(path) {
  const { port } = await api.storage.local.get({ port: DEFAULT_PORT });
  return `http://127.0.0.1:${port}${path}`;
}

// Reports a tab; returns zman's verdict on it, or null.
async function send(tab) {
  try {
    const reply = await fetch(await endpoint("/tab"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(tab),
    });
    return reply.ok ? ((await reply.json()).block ?? null) : null;
  } catch {
    // zman is not running: nothing to do, the next change will try again.
    return null;
  }
}

function blockedPage(url, block) {
  const params = new URLSearchParams({ url, site: block.site, reason: block.reason });
  if (block.until_ms) params.set("until", String(block.until_ms));
  if (block.limit_minutes) params.set("limit", String(block.limit_minutes));
  return `${api.runtime.getURL("blocked.html")}?${params}`;
}

// Sends whatever tab is active in the focused window right now.
async function report() {
  const [tab] = await api.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tab || tab.incognito || !tab.url) {
    // An empty report tells zman there is no trackable tab.
    await send({ url: "", title: "" });
    return;
  }
  const block = await send({ url: tab.url, title: tab.title ?? "" });
  if (block) await api.tabs.update(tab.id, { url: blockedPage(tab.url, block) });
}

api.tabs.onActivated.addListener(report);
api.tabs.onUpdated.addListener((_tabId, change, tab) => {
  if (tab.active && (change.url || change.title)) report();
});
api.windows.onFocusChanged.addListener((windowId) => {
  if (windowId !== api.windows.WINDOW_ID_NONE) report();
});
api.runtime.onStartup.addListener(report);
api.runtime.onInstalled.addListener(report);
api.storage.onChanged.addListener(report);

// A page that was fine a minute ago may not be now: a limit ran out, or a
// focus session started while it was open.
api.alarms.create("recheck", { periodInMinutes: 1 });
api.alarms.onAlarm.addListener(report);
