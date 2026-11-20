// zman dashboard. Plain JavaScript, no build step: the Rust side sends one
// JSON document per view (see `dashboard.rs`) and this file draws it.

const tauri = window.__TAURI__;
// Outside the app (opened in a browser) the dashboard runs on demo data.
const invoke = tauri ? tauri.core.invoke : window.zmanMock;

const TEXT = {
  en: {
    day: "Day", week: "Week", today: "Today", thisWeek: "This week",
    previous: "Previous", next: "Next", settings: "Settings",
    autostart: "Start with the system", pause: "Pause tracking",
    language: "Language", languageAuto: "Automatic",
    database: "Database", config: "Config file",
    categories: "Categories", apps: "Top apps", sites: "Top sites",
    tracked: "Time tracked", productivity: "Productivity", focus: "Focus time", topCategory: "Top category",
    vsDay: "vs the day before", vsWeek: "vs the week before", noChange: "same as before",
    blocks: (n) => (n === 1 ? "1 block of 25 min or more" : `${n} blocks of 25 min or more`),
    noFocus: "no blocks of 25 min yet",
    ofTotal: "of tracked time",
    byHour: "Activity by hour", byDay: "Activity by day",
    focusRow: "Focus",
    other: "Other",
    sitesHint: "Install the zman browser extension to see time per website.",
    nothing: "Nothing here yet",
    emptyTitle: "Nothing tracked in this period",
    emptyNow: "zman records the window you are using. Keep working and your day will show up here.",
    emptyPast: "There is no activity recorded for these dates.",
    running: "Tracking", paused: "Paused", external: "Tracking (daemon)", stopped: "Not tracking",
    points: "pts",
    connect: "Connect", close: "Close", copy: "Copy", copied: "Copied",
    connectTitle: "Connect your AI assistant",
    connectLead: "Ask about your time in plain language. Assistants only receive totals, never window titles.",
    claudeConnect: "Connect", claudeConnected: "Connected · Disconnect",
    claudeReady: "One click. Then quit Claude Desktop completely and open it again.",
    claudeDone: "Connected. Quit Claude Desktop completely (tray icon → Quit) and open it again to see the zman tools.",
    claudeMissing: "Claude Desktop is not installed on this computer.",
    claudeCodeNote: "Run this once in a terminal.",
    remoteTitle: "ChatGPT, Claude on the web and your other devices",
    remoteNote: "Sign in to a zman server to sync this device and get a link any assistant can use.",
    server: "Server", email: "Email", password: "Password",
    signIn: "Sign in", createAccount: "Create account", signOut: "Sign out", syncNow: "Sync now",
    signedIn: (email, when) => `Signed in as ${email} · last sync ${when}`,
    never: "never",
    step1: "In ChatGPT or Claude, open Settings → Connectors and add a custom connector.",
    step2: "Paste the address above.",
    step3: "Sign in with this same email and password when asked.",
  },
  es: {
    day: "Día", week: "Semana", today: "Hoy", thisWeek: "Esta semana",
    previous: "Anterior", next: "Siguiente", settings: "Ajustes",
    autostart: "Iniciar con el sistema", pause: "Pausar el registro",
    language: "Idioma", languageAuto: "Automático",
    database: "Base de datos", config: "Archivo de configuración",
    categories: "Categorías", apps: "Apps más usadas", sites: "Sitios más visitados",
    tracked: "Tiempo registrado", productivity: "Productividad", focus: "Tiempo de foco", topCategory: "Categoría principal",
    vsDay: "vs. el día anterior", vsWeek: "vs. la semana anterior", noChange: "igual que antes",
    blocks: (n) => (n === 1 ? "1 bloque de 25 min o más" : `${n} bloques de 25 min o más`),
    noFocus: "todavía sin bloques de 25 min",
    ofTotal: "del tiempo registrado",
    byHour: "Actividad por hora", byDay: "Actividad por día",
    focusRow: "Foco",
    other: "Otras",
    sitesHint: "Instalá la extensión de navegador de zman para ver el tiempo por sitio web.",
    nothing: "Todavía no hay nada",
    emptyTitle: "No hay nada registrado en este período",
    emptyNow: "zman registra la ventana que estás usando. Seguí trabajando y tu día va a aparecer acá.",
    emptyPast: "No hay actividad registrada en estas fechas.",
    running: "Registrando", paused: "En pausa", external: "Registrando (daemon)", stopped: "Sin registrar",
    points: "pts",
    connect: "Conectar", close: "Cerrar", copy: "Copiar", copied: "Copiado",
    connectTitle: "Conectá tu asistente de IA",
    connectLead: "Preguntá por tu tiempo en lenguaje natural. Los asistentes solo reciben totales, nunca títulos de ventanas.",
    claudeConnect: "Conectar", claudeConnected: "Conectado · Desconectar",
    claudeReady: "Un clic. Después cerrá Claude Desktop por completo y volvé a abrirlo.",
    claudeDone: "Conectado. Cerrá Claude Desktop por completo (ícono de la bandeja → Salir) y volvé a abrirlo para ver las herramientas de zman.",
    claudeMissing: "Claude Desktop no está instalado en esta computadora.",
    claudeCodeNote: "Ejecutá esto una vez en una terminal.",
    remoteTitle: "ChatGPT, Claude en la web y tus otros dispositivos",
    remoteNote: "Iniciá sesión en un servidor zman para sincronizar este dispositivo y obtener un enlace que sirve en cualquier asistente.",
    server: "Servidor", email: "Email", password: "Contraseña",
    signIn: "Iniciar sesión", createAccount: "Crear cuenta", signOut: "Cerrar sesión", syncNow: "Sincronizar ahora",
    signedIn: (email, when) => `Sesión iniciada como ${email} · último sync ${when}`,
    never: "nunca",
    step1: "En ChatGPT o Claude, abrí Ajustes → Conectores y agregá un conector personalizado.",
    step2: "Pegá la dirección de arriba.",
    step3: "Iniciá sesión con este mismo email y contraseña cuando te lo pida.",
  },
};

const MAX_SERIES = 8;
const state = { period: "day", date: null, data: null };
let t = TEXT.en;

const $ = (id) => document.getElementById(id);
const esc = (text) =>
  String(text).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

function duration(ms) {
  const secs = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor(secs / 60) % 60;
  if (hours > 0) return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${secs}s`;
}

const share = (ms, total) => `${Math.round((ms / Math.max(total, 1)) * 100)}%`;

// Colors come from the category rules (see `palette` in dashboard.rs), so a
// category keeps its color whatever period is on screen.
function colorOf(name) {
  const slot = state.data.category_colors[name];
  return slot ? `var(--s${slot})` : "var(--s-none)";
}

// ---------- Rendering ----------

function render(data) {
  state.data = data;
  state.date = data.date;
  t = TEXT[data.lang] || TEXT.en;
  document.documentElement.lang = data.lang;

  for (const el of document.querySelectorAll("[data-i18n]")) el.textContent = t[el.dataset.i18n];
  for (const el of document.querySelectorAll("[data-i18n-title]")) el.title = t[el.dataset.i18nTitle];
  for (const tab of document.querySelectorAll("#period-switch button")) {
    tab.setAttribute("aria-selected", String(tab.dataset.period === data.period));
  }
  $("title").textContent = data.title;
  $("next").disabled = !data.next_date;
  $("go-today").hidden = data.is_current;
  $("go-today").textContent = data.period === "day" ? t.today : t.thisWeek;
  renderStatus(data.status);

  const empty = data.total_ms === 0;
  $("app").hidden = empty;
  $("empty").hidden = !empty;
  if (empty) {
    $("empty-title").textContent = t.emptyTitle;
    $("empty-text").textContent = data.is_current ? t.emptyNow : t.emptyPast;
    return;
  }
  renderTiles(data);
  renderChart(data);
  renderList("categories", data.categories.map((c) => ({ name: c.name, ms: c.ms, color: colorOf(c.name) })), data.total_ms);
  renderList("apps", data.apps.map((a) => ({ name: a.name, note: a.category, ms: a.ms, color: colorOf(a.category) })), data.total_ms);
  renderList("sites", data.sites.map((s) => ({ name: s.name, note: s.category, ms: s.ms, color: colorOf(s.category) })), data.total_ms, t.sitesHint);
}

function renderStatus(status) {
  $("status").dataset.state = status.tracking;
  $("status-text").textContent = t[status.tracking] || t.stopped;
  $("autostart").checked = status.autostart;
  $("paused").checked = status.tracking === "paused";
  $("paused").disabled = status.tracking === "external" || status.tracking === "stopped";
  $("language").value = status.language || "auto";
  $("db-path").textContent = status.db_path;
  $("config-path").textContent = status.config_path;
}

function delta(current, previous, format, goodWhenUp) {
  const versus = state.data.period === "day" ? t.vsDay : t.vsWeek;
  if (previous === null || previous === undefined || current === null) return "";
  const diff = current - previous;
  if (diff === 0) return esc(t.noChange);
  const arrow = diff > 0 ? "▲" : "▼";
  const tone = goodWhenUp ? (diff > 0 ? "delta-up" : "delta-down") : "";
  return `<span class="${tone}">${arrow} ${esc(format(Math.abs(diff)))}</span> ${esc(versus)}`;
}

function renderTiles(d) {
  const prev = d.previous;
  const hasPrev = prev.total_ms > 0;
  const top = d.categories[0];
  const tiles = [
    {
      label: t.tracked,
      value: esc(duration(d.total_ms)),
      note: hasPrev ? delta(d.total_ms, prev.total_ms, duration, false) : "",
    },
    {
      label: t.productivity,
      value: `${d.productivity}<small> / 100</small>`,
      note: hasPrev ? delta(d.productivity, prev.productivity, (n) => `${n} ${t.points}`, true) : "",
      meter: d.productivity,
    },
    {
      label: t.focus,
      value: esc(duration(d.focus_ms)),
      note: esc(d.focus_blocks.length ? t.blocks(d.focus_blocks.length) : t.noFocus),
    },
    {
      label: t.topCategory,
      value: `<span class="swatch" style="background:${colorOf(top.name)};margin-right:9px"></span>${esc(top.name)}`,
      note: `${share(top.ms, d.total_ms)} ${esc(t.ofTotal)} · ${esc(duration(top.ms))}`,
      small: true,
    },
  ];
  $("tiles").innerHTML = tiles
    .map(
      (tile) => `
      <div class="tile">
        <div class="tile-label">${esc(tile.label)}</div>
        <div class="tile-value"${tile.small ? ' style="font-size:22px;line-height:1.63"' : ""}>${tile.value}</div>
        ${tile.meter === undefined ? "" : `<div class="meter"><div style="width:${tile.meter}%"></div></div>`}
        <div class="tile-note">${tile.note}</div>
      </div>`,
    )
    .join("");
}

// Stacked columns: one per hour (day view) or per day (week view).
function renderChart(d) {
  const isDay = d.period === "day";
  $("chart-title").textContent = isDay ? t.byHour : t.byDay;

  // Stacking order matches the legend. Categories without a color of their
  // own (past the eighth) fold into one gray "Other" segment.
  const present = d.category_order.filter((name) => d.categories.some((c) => c.name === name));
  const series = present.filter((name) => d.category_colors[name] || present.length <= MAX_SERIES);
  const folded = present.filter((name) => !series.includes(name));
  $("legend").innerHTML =
    series.map((name) => `<span><i class="swatch" style="background:${colorOf(name)}"></i>${esc(name)}</span>`).join("") +
    (folded.length ? `<span><i class="swatch" style="background:var(--s-none)"></i>${esc(t.other)}</span>` : "");

  // Y scale: an hour holds 60 minutes; a day gets a rounded number of hours.
  let top, ticks;
  if (isDay) {
    top = 3600000;
    ticks = [0, 15, 30, 45, 60].map((m) => ({ ms: m * 60000, label: m === 0 ? "0" : `${m}m` }));
  } else {
    const maxHours = Math.max(...d.buckets.map((b) => b.total_ms)) / 3600000;
    const step = [1, 2, 3, 4, 6].find((s) => maxHours / s <= 4) || 8;
    const count = Math.max(1, Math.ceil(maxHours / step));
    top = count * step * 3600000;
    ticks = Array.from({ length: count + 1 }, (_, i) => ({ ms: i * step * 3600000, label: i === 0 ? "0" : `${i * step}h` }));
  }
  const pct = (ms) => (ms / top) * 100;

  const columns = d.buckets
    .map((bucket, index) => {
      const parts = series.map((name) => ({ name, ms: bucket.parts[name] || 0, color: colorOf(name) }));
      const rest = bucket.total_ms - parts.reduce((sum, p) => sum + p.ms, 0);
      if (rest > 0) parts.push({ name: t.other, ms: rest, color: "var(--s-none)" });
      // Slivers under 1% of the scale would only add noise.
      const segments = parts
        .filter((p) => pct(p.ms) >= 1)
        .map((p) => `<div class="seg" style="height:${pct(p.ms).toFixed(2)}%;background:${p.color}"></div>`)
        .join("");
      return `<div class="col" data-bucket="${index}">${segments}</div>`;
    })
    .join("");

  const labels = d.buckets
    .map((bucket) => `<span class="${bucket.is_now ? "is-now" : ""}">${bucket.show_label ? esc(bucket.label) : ""}</span>`)
    .join("");
  const grid = `grid-template-columns:repeat(${d.buckets.length},minmax(0,1fr))`;

  let focus = "";
  if (isDay) {
    const span = d.range_end_ms - d.range_start_ms;
    const blocks = d.focus_blocks
      .map((block, index) => {
        const left = ((block.start_ms - d.range_start_ms) / span) * 100;
        const width = ((block.end_ms - block.start_ms) / span) * 100;
        return `<div class="focus-block" data-focus="${index}" style="left:${left.toFixed(3)}%;width:${width.toFixed(3)}%"></div>`;
      })
      .join("");
    focus = `<div class="focus-row"><span>${esc(t.focusRow)}</span><div class="focus-track">${blocks}</div></div>`;
  }

  $("chart").innerHTML = `
    <div class="axis-y">${ticks.map((tick) => `<span style="bottom:${pct(tick.ms)}%">${tick.label}</span>`).join("")}</div>
    <div class="plot">
      ${ticks.slice(1).map((tick) => `<div class="gridline" style="bottom:${pct(tick.ms)}%"></div>`).join("")}
      <div class="cols" style="${grid}">${columns}</div>
    </div>
    <div class="axis-x" style="${grid}">${labels}</div>
    ${focus}`;
}

function renderList(id, rows, total, hint) {
  const longest = rows.length ? rows[0].ms : 1;
  $(id).innerHTML = rows.length
    ? rows
        .map(
          (row) => `
        <div class="row">
          <div class="row-name">
            <span class="swatch" style="background:${row.color}"></span>
            <b title="${esc(row.name)}">${esc(row.name)}</b>
            ${row.note ? `<em>${esc(row.note)}</em>` : ""}
          </div>
          <div class="row-value">${esc(duration(row.ms))}<small>${share(row.ms, total)}</small></div>
          <div class="row-bar"><div style="width:${((row.ms / longest) * 100).toFixed(2)}%;background:${row.color}"></div></div>
        </div>`,
        )
        .join("")
    : `<p class="hint">${esc(hint || t.nothing)}</p>`;
}

// ---------- Tooltip ----------

function tooltipFor(target) {
  const d = state.data;
  const column = target.closest("[data-bucket]");
  if (column) {
    const bucket = d.buckets[Number(column.dataset.bucket)];
    if (bucket.total_ms === 0) return null;
    const rows = d.categories
      .map((c) => ({ name: c.name, ms: bucket.parts[c.name] || 0 }))
      .filter((p) => p.ms >= 1000)
      .sort((a, b) => b.ms - a.ms)
      .map((p) => `<div class="tooltip-row"><i class="swatch" style="background:${colorOf(p.name)}"></i><span>${esc(p.name)}</span><span>${esc(duration(p.ms))}</span></div>`)
      .join("");
    const score = bucket.productivity === null ? "" : `<div class="tooltip-row"><span>${esc(t.productivity)}</span><span>${bucket.productivity}</span></div>`;
    return `<div class="tooltip-title"><span>${esc(bucket.tooltip)}</span><span>${esc(duration(bucket.total_ms))}</span></div>${rows}${score}`;
  }
  const focus = target.closest("[data-focus]");
  if (focus) {
    const block = d.focus_blocks[Number(focus.dataset.focus)];
    return `<div class="tooltip-title"><span>${esc(block.start_label)} – ${esc(block.end_label)}</span><span>${esc(duration(block.end_ms - block.start_ms))}</span></div>
      <div class="tooltip-row"><i class="swatch" style="background:${colorOf(block.category)}"></i><span>${esc(block.category)}</span></div>`;
  }
  return null;
}

document.addEventListener("mousemove", (event) => {
  const tooltip = $("tooltip");
  const html = state.data ? tooltipFor(event.target) : null;
  if (!html) {
    tooltip.hidden = true;
    return;
  }
  tooltip.innerHTML = html;
  tooltip.hidden = false;
  // Keep the tooltip inside the window, flipping to the left of the cursor if needed.
  const box = tooltip.getBoundingClientRect();
  let x = event.clientX + 14;
  if (x + box.width > window.innerWidth - 8) x = event.clientX - box.width - 14;
  const y = Math.min(event.clientY + 14, window.innerHeight - box.height - 8);
  tooltip.style.left = `${Math.max(8, x)}px`;
  tooltip.style.top = `${Math.max(8, y)}px`;
});

// ---------- Loading & controls ----------

async function load() {
  try {
    render(await invoke("get_dashboard", { period: state.period, date: state.date }));
  } catch (error) {
    $("app").hidden = true;
    $("empty").hidden = false;
    $("empty-title").textContent = "zman";
    $("empty-text").textContent = String(error);
  }
}

$("period-switch").addEventListener("click", (event) => {
  const period = event.target.dataset.period;
  if (!period || period === state.period) return;
  state.period = period;
  load();
});
$("prev").addEventListener("click", () => { state.date = state.data.prev_date; load(); });
$("next").addEventListener("click", () => { if (state.data.next_date) { state.date = state.data.next_date; load(); } });
$("go-today").addEventListener("click", () => { state.date = null; load(); });

const menu = $("settings-menu");
$("settings-btn").addEventListener("click", (event) => {
  event.stopPropagation();
  menu.hidden = !menu.hidden;
  $("settings-btn").setAttribute("aria-expanded", String(!menu.hidden));
});
document.addEventListener("click", (event) => {
  if (!menu.hidden && !menu.contains(event.target)) menu.hidden = true;
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") menu.hidden = true;
  if (event.target.tagName === "INPUT") return;
  if (event.key === "ArrowLeft") $("prev").click();
  if (event.key === "ArrowRight") $("next").click();
});
$("autostart").addEventListener("change", async (event) => {
  renderStatus(await invoke("set_autostart", { enabled: event.target.checked }));
});
$("language").addEventListener("change", async (event) => {
  await invoke("set_language", { language: event.target.value });
  // Everything on screen, including dates and category names, is redrawn.
  load();
});
$("paused").addEventListener("change", async (event) => {
  renderStatus(await invoke("set_paused", { paused: event.target.checked }));
});

// ---------- Connect dialog ----------

const dialog = $("connect-dialog");

function renderConnections(c) {
  const claude = c.claude_desktop;
  const toggle = $("claude-toggle");
  toggle.disabled = !claude.installed;
  toggle.textContent = claude.connected ? t.claudeConnected : t.claudeConnect;
  toggle.classList.toggle("is-on", claude.connected);
  toggle.dataset.connected = String(claude.connected);
  $("claude-note").textContent = !claude.installed ? t.claudeMissing : claude.connected ? t.claudeDone : t.claudeReady;
  $("claude-code-command").textContent = c.claude_code_command;

  $("sync-form").hidden = Boolean(c.sync);
  $("sync-account").hidden = !c.sync;
  if (c.sync) {
    const when = c.sync.last_sync_ms
      ? new Date(c.sync.last_sync_ms).toLocaleString(state.data.lang, { dateStyle: "medium", timeStyle: "short" })
      : t.never;
    $("sync-summary").textContent = t.signedIn(c.sync.email, when);
    $("mcp-url").textContent = c.sync.mcp_url;
  }
}

// Runs a connection command, showing its error (if any) inside the dialog.
async function connectionCommand(command, args, button) {
  const error = $("connect-error");
  error.hidden = true;
  if (button) button.disabled = true;
  try {
    renderConnections(await invoke(command, args));
  } catch (problem) {
    error.textContent = String(problem);
    error.hidden = false;
  } finally {
    if (button) button.disabled = false;
  }
}

$("connect-btn").addEventListener("click", async () => {
  $("connect-error").hidden = true;
  dialog.showModal();
  await connectionCommand("get_connections", {});
});
$("connect-close").addEventListener("click", () => dialog.close());
dialog.addEventListener("click", (event) => { if (event.target === dialog) dialog.close(); });

$("claude-toggle").addEventListener("click", (event) => {
  const connected = event.currentTarget.dataset.connected !== "true";
  connectionCommand("set_claude_desktop", { connected }, event.currentTarget);
});
$("sync-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const args = {
    server: $("sync-server").value,
    email: $("sync-email").value,
    password: $("sync-password").value,
    register: event.submitter?.dataset.register === "true",
  };
  connectionCommand("sync_login", args, event.submitter).then(() => { $("sync-password").value = ""; });
});
$("sync-now").addEventListener("click", (event) => connectionCommand("sync_now", {}, event.currentTarget));
$("sync-logout").addEventListener("click", (event) => connectionCommand("sync_logout", {}, event.currentTarget));

for (const button of document.querySelectorAll("[data-copy]")) {
  button.addEventListener("click", async () => {
    await navigator.clipboard.writeText($(button.dataset.copy).textContent);
    button.textContent = t.copied;
    setTimeout(() => { button.textContent = t.copy; }, 1500);
  });
}

// The current day or week keeps itself up to date while the window is open.
setInterval(() => {
  if (!document.hidden && state.data && state.data.is_current) load();
}, 30000);
document.addEventListener("visibilitychange", () => { if (!document.hidden) load(); });
if (tauri) tauri.event.listen("status-changed", load);

load();
