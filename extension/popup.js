const api = globalThis.browser ?? globalThis.chrome;
const DEFAULT_PORT = 27184;

const spanish = (navigator.language || "").toLowerCase().startsWith("es");
const t = spanish
  ? {
      on: "Conectado a Tu Hora Vale",
      off: "Tu Hora Vale no está corriendo",
      onHint: "El sitio de la pestaña activa se suma a tu registro. Las ventanas privadas nunca se informan.",
      offHint: "Abrí la app de Tu Hora Vale en esta computadora.",
      advanced: "Avanzado",
      port: "Puerto local",
    }
  : {
      on: "Connected to Tu Hora Vale",
      off: "Tu Hora Vale is not running",
      onHint: "The active tab's site is added to your tracking. Private windows are never reported.",
      offHint: "Open the Tu Hora Vale app on this computer.",
      advanced: "Advanced",
      port: "Local port",
    };

const $ = (id) => document.getElementById(id);
$("advanced").textContent = t.advanced;
$("port-label").textContent = t.port;

async function check() {
  const { port } = await api.storage.local.get({ port: DEFAULT_PORT });
  $("port").value = port;
  let connected = false;
  try {
    const reply = await fetch(`http://127.0.0.1:${port}/status`);
    connected = reply.ok && (await reply.json()).app === "zman";
  } catch {
    connected = false;
  }
  $("status").classList.toggle("on", connected);
  $("status-text").textContent = connected ? t.on : t.off;
  $("hint").textContent = connected ? t.onHint : t.offHint;
}

$("port").addEventListener("change", async (event) => {
  const port = Number(event.target.value);
  if (Number.isInteger(port) && port >= 1 && port <= 65535) {
    await api.storage.local.set({ port });
    check();
  }
});

check();
