// zman browser extension: reports the active tab's URL and title to the zman
// tracker running on this computer (127.0.0.1). It never talks to any other
// host, and it never reports private/incognito tabs.

const api = globalThis.browser ?? globalThis.chrome;
const DEFAULT_PORT = 27184;

async function endpoint(path) {
  const { port } = await api.storage.local.get({ port: DEFAULT_PORT });
  return `http://127.0.0.1:${port}${path}`;
}

async function send(tab) {
  try {
    await fetch(await endpoint("/tab"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(tab),
    });
  } catch {
    // zman is not running: nothing to do, the next change will try again.
  }
}

// Sends whatever tab is active in the focused window right now.
async function report() {
  const [tab] = await api.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tab || tab.incognito || !tab.url) {
    // An empty report tells zman there is no trackable tab.
    await send({ url: "", title: "" });
    return;
  }
  await send({ url: tab.url, title: tab.title ?? "" });
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
