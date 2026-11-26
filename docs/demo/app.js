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
    sitesHint: "Sites show up here as you browse with Chrome, Edge, Brave or Firefox.",
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
    focusBtn: "Focus", close2: "Close",
    focusTitle: "Focus and blocking",
    focusLead: "Keep distracting sites away while you work.",
    sessionTitle: "Focus session",
    sessionNote: "Blocks distracting sites (entertainment, social media) until the time is up.",
    endSession: "End session",
    focusEnds: (time) => `Distracting sites are blocked until ${time}.`,
    limitsTitle: "Daily limits",
    limitsNote: "A site or a whole category is blocked for the rest of the day once its time runs out.",
    minPerDay: "min per day", add: "Add", remove: "Remove",
    limitRow: (minutes) => `${minutes} min per day`,
    hoursTitle: "Focus hours",
    hoursNote: "Distracting sites are blocked every week at these hours.",
    dayLetters: ["M", "T", "W", "T", "F", "S", "S"],
    dayNames: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    sitesTitle: "Always blocked",
    sitesNote: "Sites you never want to open on this computer.",
    goalTitle: "Daily goal",
    goalNote: "Hours of productive time you want to reach each day. 0 turns it off.",
    hoursPerDay: "hours per day",
    notifications: "Notify me about goals, long distractions and my weekly recap",
    assistantFocus: "Let my AI assistant start focus sessions",
    extensionNote: "Blocking works in browsers that have the zman extension installed.",
    goal: "Daily goal", goalDone: "Goal reached", goalLeft: (left) => `${left} to go`,
    productiveToday: "of productive time",
    moveTo: (name) => `${name} goes to`, automatic: "Automatic",
    exportCsv: "Export this view as CSV", exported: (path) => `Saved to ${path}`,
    clientsTitle: "Clients", clientsEdit: "Edit clients",
    clientsEmpty: "Tell zman who you work for and it will show how many hours each client takes and what you really earn per hour. No timers: it recognises the project folder, the client's site or a word in the window title.",
    clientsStart: "Add my clients",
    clientCol: "Client", periodDay: "Today", periodWeek: "This week", monthCol: (m) => m,
    hoursCol: "Hours", paidCol: "Paid", perHourCol: "Per hour",
    ownTag: "own", unassigned: "No client",
    clientsDialogTitle: "Your clients",
    clientsDialogLead: "zman adds up the time of each client by itself. Add what they paid and it tells you your real rate per hour.",
    clientAdd: "Add a client", clientName: "Name", clientOwn: "My own project (counts hours, no rate)",
    clientKeywords: "Words in the window title", clientKeywordsHint: "project folder, company name… separated by commas",
    clientDomains: "Websites", clientApps: "Apps", clientDelete: "Delete client",
    paymentsTitle: "Payments", paymentAdd: "Add payment", paymentNote: "Note (invoice number…)",
    suggestionsTitle: "Projects zman saw this month with no client — click to add",
    clientsPrivacy: "Clients and payments stay on this computer. Your AI assistant can read them through zman (\"how much did I earn per hour with X?\") and save payments you tell it about.",
    newClient: "New client",
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
    sitesHint: "Los sitios aparecen acá a medida que navegás con Chrome, Edge, Brave o Firefox.",
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
    focusBtn: "Foco", close2: "Cerrar",
    focusTitle: "Foco y bloqueo",
    focusLead: "Mantené lejos los sitios que distraen mientras trabajás.",
    sessionTitle: "Sesión de foco",
    sessionNote: "Bloquea los sitios que distraen (entretenimiento, redes sociales) hasta que termine el tiempo.",
    endSession: "Terminar sesión",
    focusEnds: (time) => `Los sitios que distraen están bloqueados hasta las ${time}.`,
    limitsTitle: "Límites diarios",
    limitsNote: "Un sitio o una categoría entera se bloquea por el resto del día cuando se le acaba el tiempo.",
    minPerDay: "min por día", add: "Agregar", remove: "Quitar",
    limitRow: (minutes) => `${minutes} min por día`,
    hoursTitle: "Horario de foco",
    hoursNote: "Los sitios que distraen se bloquean todas las semanas en estos horarios.",
    dayLetters: ["L", "M", "M", "J", "V", "S", "D"],
    dayNames: ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"],
    sitesTitle: "Siempre bloqueados",
    sitesNote: "Sitios que no querés abrir nunca en esta computadora.",
    goalTitle: "Meta diaria",
    goalNote: "Horas de tiempo productivo que querés alcanzar por día. Con 0 se apaga.",
    hoursPerDay: "horas por día",
    notifications: "Avisarme de metas, distracciones largas y mi resumen semanal",
    assistantFocus: "Dejar que mi asistente de IA inicie sesiones de foco",
    extensionNote: "El bloqueo funciona en los navegadores que tienen instalada la extensión de zman.",
    goal: "Meta diaria", goalDone: "Meta cumplida", goalLeft: (left) => `faltan ${left}`,
    productiveToday: "de tiempo productivo",
    moveTo: (name) => `${name} va a`, automatic: "Automática",
    exportCsv: "Exportar esta vista a CSV", exported: (path) => `Guardado en ${path}`,
    clientsTitle: "Clientes", clientsEdit: "Editar clientes",
    clientsEmpty: "Decile a zman para quién trabajás y te muestra cuántas horas te lleva cada cliente y cuánto ganás de verdad por hora. Sin cronómetros: reconoce la carpeta del proyecto, el sitio del cliente o una palabra en el título de la ventana.",
    clientsStart: "Agregar mis clientes",
    clientCol: "Cliente", periodDay: "Hoy", periodWeek: "Esta semana", monthCol: (m) => m,
    hoursCol: "Horas", paidCol: "Cobrado", perHourCol: "Por hora",
    ownTag: "propio", unassigned: "Sin cliente",
    clientsDialogTitle: "Tus clientes",
    clientsDialogLead: "zman suma solo el tiempo de cada cliente. Cargá lo que te pagaron y te dice cuánto ganás de verdad por hora.",
    clientAdd: "Agregar cliente", clientName: "Nombre", clientOwn: "Proyecto propio (cuenta horas, sin tarifa)",
    clientKeywords: "Palabras en el título de la ventana", clientKeywordsHint: "carpeta del proyecto, nombre de la empresa… separadas por comas",
    clientDomains: "Sitios web", clientApps: "Apps", clientDelete: "Borrar cliente",
    paymentsTitle: "Cobros", paymentAdd: "Agregar cobro", paymentNote: "Nota (número de factura…)",
    suggestionsTitle: "Proyectos que zman vio este mes sin cliente — tocá para agregar",
    clientsPrivacy: "Los clientes y los cobros quedan en esta computadora. Tu asistente de IA los puede leer a través de zman (\"¿cuánto gané por hora con X?\") y anotar los cobros que le cuentes.",
    newClient: "Cliente nuevo",
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
  renderFocusButton();

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
  loadClients();
  renderList("categories", data.categories.map((c) => ({ name: c.name, ms: c.ms, color: colorOf(c.name) })), data.total_ms);
  renderList("apps", data.apps.map((a) => ({ kind: "app", id: a.id, name: a.name, note: a.category, ms: a.ms, color: colorOf(a.category) })), data.total_ms);
  renderList("sites", data.sites.map((s) => ({ kind: "site", id: s.id, name: s.name, note: s.category, ms: s.ms, color: colorOf(s.category) })), data.total_ms, t.sitesHint);
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
    goalTile(d) || {
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

// With a daily goal set, the day view shows progress towards it.
function goalTile(d) {
  if (!d.goal_minutes || d.period !== "day") return null;
  const goal = d.goal_minutes * 60000;
  const done = d.categories.filter((c) => c.score > 0).reduce((sum, c) => sum + c.ms, 0);
  return {
    label: t.goal,
    value: `${esc(duration(done))}<small> / ${esc(duration(goal))}</small>`,
    note: esc(done >= goal ? t.goalDone : `${t.goalLeft(duration(goal - done))} · ${t.productiveToday}`),
    meter: Math.min(100, Math.round((done / goal) * 100)),
  };
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
        <div class="row"${row.kind && row.id ? ` data-kind="${row.kind}" data-id="${esc(row.id)}" data-name="${esc(row.name)}" data-category="${esc(row.note)}"` : ""}>
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

// ---------- Focus and blocking ----------

const focusDialog = $("focus-dialog");
const pad = (n) => String(n).padStart(2, "0");
const clockOf = (ms) => new Date(ms).toLocaleTimeString(state.data.lang, { hour: "2-digit", minute: "2-digit" });
const minuteLabel = (min) => `${pad(Math.floor(min / 60))}:${pad(min % 60)}`;

function countdown(until) {
  const secs = Math.max(0, Math.ceil((until - Date.now()) / 1000));
  const hours = Math.floor(secs / 3600);
  return hours ? `${hours}:${pad(Math.floor(secs / 60) % 60)}:${pad(secs % 60)}` : `${Math.floor(secs / 60)}:${pad(secs % 60)}`;
}

// The button in the top bar and the clock in the dialog, once a second.
function renderFocusButton() {
  if (!state.data) return;
  const until = state.data.focus_until_ms || 0;
  const running = until > Date.now();
  $("focus-btn").classList.toggle("is-on", running);
  $("focus-label").textContent = running ? countdown(until) : t.focusBtn;
  $("focus-idle").hidden = running;
  $("focus-running").hidden = !running;
  if (running) {
    $("focus-clock").textContent = countdown(until);
    $("focus-ends").textContent = t.focusEnds(clockOf(until));
  } else if (until) {
    // The session just ended.
    state.data.focus_until_ms = 0;
    load();
  }
}
setInterval(renderFocusButton, 1000);

const removeIcon = '<svg viewBox="0 0 16 16" width="13" height="13"><path d="m4 4 8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>';
const ruleRow = (list, index, html) =>
  `<div class="rule">${html}<button class="icon-btn" data-remove="${list}" data-index="${index}" title="${esc(t.remove)}">${removeIcon}</button></div>`;

function renderFocus(reply) {
  state.settings = reply.settings;
  state.data.focus_until_ms = reply.settings.block.focus_until_ms > Date.now() ? reply.settings.block.focus_until_ms : 0;
  state.data.goal_minutes = reply.settings.goal_minutes;
  const block = reply.settings.block;

  $("limit-list").innerHTML = block.limits
    .map((limit, i) => ruleRow("limits", i, `<b>${esc(limit.target)}</b><span>${esc(t.limitRow(limit.minutes))}</span>`))
    .join("");
  $("limit-options").innerHTML = reply.categories
    .filter((c) => c.score < 0)
    .map((c) => `<option value="${esc(c.name)}">`)
    .join("");
  $("schedule-list").innerHTML = block.schedules
    .map((s, i) => {
      const days = s.days.length === 7 ? "" : `<span>${esc([...s.days].sort().map((d) => t.dayNames[d - 1]).join(" "))}</span>`;
      return ruleRow("schedules", i, `<b>${minuteLabel(s.start_min)} – ${minuteLabel(s.end_min)}</b>${days}`);
    })
    .join("");
  $("site-list").innerHTML = block.sites.map((site, i) => ruleRow("sites", i, `<b>${esc(site)}</b>`)).join("");

  if (!$("schedule-days").children.length || $("schedule-days").dataset.lang !== state.data.lang) {
    $("schedule-days").dataset.lang = state.data.lang;
    $("schedule-days").innerHTML = t.dayLetters
      .map((letter, i) => `<button type="button" data-day="${i + 1}" aria-pressed="${i < 5}" title="${t.dayNames[i]}">${letter}</button>`)
      .join("");
  }
  if (document.activeElement !== $("goal-hours")) $("goal-hours").value = reply.settings.goal_minutes / 60;
  $("notifications").checked = reply.settings.notifications;
  $("assistant-focus").checked = reply.settings.assistant_focus;
  renderFocusButton();
}

async function focusCommand(command, args) {
  const error = $("focus-error");
  error.hidden = true;
  try {
    renderFocus(await invoke(command, args));
    if (command !== "get_settings") load();
  } catch (problem) {
    error.textContent = String(problem);
    error.hidden = false;
  }
}
const saveSettings = () => focusCommand("save_settings", { settings: state.settings });

$("focus-btn").addEventListener("click", () => {
  focusDialog.showModal();
  focusCommand("get_settings", {});
});
$("focus-close").addEventListener("click", () => focusDialog.close());
focusDialog.addEventListener("click", (event) => {
  if (event.target === focusDialog) return focusDialog.close();
  const remove = event.target.closest("[data-remove]");
  if (remove && state.settings) {
    state.settings.block[remove.dataset.remove].splice(Number(remove.dataset.index), 1);
    saveSettings();
  }
});
$("focus-start").addEventListener("click", (event) => {
  const minutes = Number(event.target.dataset.minutes);
  if (minutes) focusCommand("set_focus", { minutes });
});
$("focus-stop").addEventListener("click", () => focusCommand("set_focus", { minutes: 0 }));
$("schedule-days").addEventListener("click", (event) => {
  const day = event.target.closest("[data-day]");
  if (day) day.setAttribute("aria-pressed", String(day.getAttribute("aria-pressed") !== "true"));
});
$("limit-form").addEventListener("submit", (event) => {
  event.preventDefault();
  state.settings.block.limits.push({ target: $("limit-target").value.trim(), minutes: Number($("limit-minutes").value) });
  $("limit-target").value = "";
  saveSettings();
});
$("schedule-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const minutes = (id) => { const [h, m] = $(id).value.split(":").map(Number); return h * 60 + m; };
  const days = [...$("schedule-days").querySelectorAll('[aria-pressed="true"]')].map((b) => Number(b.dataset.day));
  state.settings.block.schedules.push({ days, start_min: minutes("schedule-start"), end_min: minutes("schedule-end") });
  saveSettings();
});
$("site-form").addEventListener("submit", (event) => {
  event.preventDefault();
  state.settings.block.sites.push($("site-input").value.trim());
  $("site-input").value = "";
  saveSettings();
});
$("goal-hours").addEventListener("change", (event) => {
  state.settings.goal_minutes = Math.round(Math.max(0, Number(event.target.value) || 0) * 60);
  saveSettings();
});
$("notifications").addEventListener("change", (event) => { state.settings.notifications = event.target.checked; saveSettings(); });
$("assistant-focus").addEventListener("change", (event) => { state.settings.assistant_focus = event.target.checked; saveSettings(); });

// ---------- Moving an app or site to another category ----------

const catMenu = $("cat-menu");

function openCategoryMenu(row, event) {
  const { kind, id, name, category } = row.dataset;
  const options = state.data.category_order
    .map((c) => `<button role="menuitemradio" aria-checked="${c === category}" data-category="${esc(c)}"><i class="swatch" style="background:${colorOf(c)}"></i>${esc(c)}</button>`)
    .join("");
  catMenu.innerHTML = `<h4>${esc(t.moveTo(name))}</h4>${options}<hr><button data-category="">${esc(t.automatic)}</button>`;
  catMenu.dataset.kind = kind;
  catMenu.dataset.id = id;
  catMenu.hidden = false;
  const box = catMenu.getBoundingClientRect();
  catMenu.style.left = `${Math.max(8, Math.min(event.clientX, window.innerWidth - box.width - 8))}px`;
  catMenu.style.top = `${Math.max(8, Math.min(event.clientY + 6, window.innerHeight - box.height - 8))}px`;
}

document.addEventListener("click", async (event) => {
  const choice = event.target.closest("#cat-menu button");
  if (choice) {
    catMenu.hidden = true;
    await invoke("set_category", { kind: catMenu.dataset.kind, name: catMenu.dataset.id, category: choice.dataset.category || null });
    return load();
  }
  const row = event.target.closest(".row[data-kind]");
  catMenu.hidden = true;
  if (row) openCategoryMenu(row, event);
});
document.addEventListener("keydown", (event) => { if (event.key === "Escape") catMenu.hidden = true; });

// ---------- Clients ----------

const clientsDialog = $("clients-dialog");
const money = (list) =>
  list.map((m) => new Intl.NumberFormat(state.data.lang, { style: "currency", currency: m.currency, maximumFractionDigits: 0 }).format(m.amount)).join(" + ");
const rangeArgs = () => ({ startMs: state.data.range_start_ms, endMs: state.data.range_end_ms });

async function loadClients() {
  try {
    renderClients(await invoke("get_clients", rangeArgs()));
  } catch (problem) {
    $("clients-body").innerHTML = `<p class="hint">${esc(String(problem))}</p>`;
  }
}

function renderClients(reply) {
  state.clients = reply;
  const body = $("clients-body");
  $("clients-edit").hidden = !reply.clients.length;
  if (!reply.clients.length) {
    body.innerHTML = `<div class="clients-empty"><p class="hint">${esc(t.clientsEmpty)}</p>
      <button class="primary-btn" id="clients-start">${esc(t.clientsStart)}</button></div>`;
  } else {
    const month = new Date(`${reply.month_start}T12:00:00`).toLocaleDateString(state.data.lang, { month: "long" });
    const byName = Object.fromEntries(reply.period.clients.map((c) => [c.name, c]));
    const names = [...new Set([...reply.month.clients.map((c) => c.name), ...reply.period.clients.map((c) => c.name)])];
    const longest = Math.max(1, ...reply.month.clients.map((c) => c.ms));
    const rows = names.map((name) => {
      const m = reply.month.clients.find((c) => c.name === name) || { ms: 0, paid: [], per_hour: [], own: (byName[name] || {}).own };
      const p = byName[name] || { ms: 0 };
      return `<tr>
        <td><b>${esc(name)}</b>${m.own ? ` <em class="tag">${esc(t.ownTag)}</em>` : ""}
          <div class="row-bar"><div style="width:${((m.ms / longest) * 100).toFixed(1)}%;background:${m.own ? "var(--s-none)" : "var(--accent)"}"></div></div></td>
        <td>${p.ms ? esc(duration(p.ms)) : "—"}</td>
        <td>${m.ms ? esc(duration(m.ms)) : "—"}</td>
        <td>${m.paid.length ? esc(money(m.paid)) : "—"}</td>
        <td class="rate">${m.per_hour.length ? esc(money(m.per_hour)) : "—"}</td>
      </tr>`;
    });
    rows.push(`<tr class="muted"><td>${esc(t.unassigned)}</td><td>${esc(duration(reply.period.unassigned_ms))}</td><td>${esc(duration(reply.month.unassigned_ms))}</td><td></td><td></td></tr>`);
    body.innerHTML = `<table class="ctable"><thead><tr>
      <th>${esc(t.clientCol)}</th><th>${esc(state.data.period === "day" ? t.periodDay : t.periodWeek)}</th>
      <th>${esc(month)} · ${esc(t.hoursCol)}</th><th>${esc(t.paidCol)}</th><th>${esc(t.perHourCol)}</th>
    </tr></thead><tbody>${rows.join("")}</tbody></table>`;
  }
  if (clientsDialog.open) renderClientEditors();
}

const list = (text) => text.split(",").map((s) => s.trim()).filter(Boolean);
const today = () => new Date().toISOString().slice(0, 10);

function renderClientEditors() {
  const reply = state.clients;
  $("client-suggestions").innerHTML = reply.suggestions.length
    ? `<p class="hint">${esc(t.suggestionsTitle)}</p><div class="rule-list chips">${reply.suggestions
        .map((s) => `<button class="chip-btn" data-suggest="${esc(s.name)}">+ ${esc(s.name)} <small>${esc(duration(s.ms))}</small></button>`)
        .join("")}</div>`
    : "";
  $("client-editors").innerHTML = reply.clients
    .map(
      (c, i) => `<section class="conn client-edit" data-client="${i}">
      <div class="add-row">
        <input class="grow name" data-f="name" value="${esc(c.name)}" placeholder="${esc(t.clientName)}">
        <button class="icon-btn" data-del-client="${i}" title="${esc(t.clientDelete)}">${removeIcon}</button>
      </div>
      <label class="switch-row flush"><span>${esc(t.clientOwn)}</span><input type="checkbox" data-f="own" ${c.own ? "checked" : ""}><span class="switch"></span></label>
      <label class="field"><span>${esc(t.clientKeywords)}</span><input data-f="keywords" value="${esc(c.keywords.join(", "))}" placeholder="${esc(t.clientKeywordsHint)}"></label>
      <div class="field-pair">
        <label class="field"><span>${esc(t.clientDomains)}</span><input data-f="domains" value="${esc(c.domains.join(", "))}" placeholder="cliente.com"></label>
        <label class="field"><span>${esc(t.clientApps)}</span><input data-f="apps" value="${esc(c.apps.join(", "))}" placeholder="app.exe"></label>
      </div>
      ${c.own ? "" : `<h3 class="sub">${esc(t.paymentsTitle)}</h3>
      <div class="rule-list">${c.payments
        .map((p, j) => `<div class="rule"><b>${esc(money([p]))}</b><span>${esc(p.date)}${p.note ? ` · ${esc(p.note)}` : ""}</span>
          <button class="icon-btn" data-del-payment="${i}:${j}" title="${esc(t.remove)}">${removeIcon}</button></div>`)
        .join("")}</div>
      <form class="add-row payment-form" data-pay="${i}">
        <input type="date" name="date" value="${today()}" required>
        <input type="number" name="amount" class="narrow wide" min="0" step="any" placeholder="0" required>
        <select name="currency"><option>ARS</option><option>USD</option><option>EUR</option></select>
        <input name="note" class="grow" placeholder="${esc(t.paymentNote)}">
        <button class="ghost-btn">${esc(t.paymentAdd)}</button>
      </form>`}
    </section>`,
    )
    .join("");
}

async function saveClients() {
  const error = $("clients-error");
  error.hidden = true;
  try {
    renderClients(await invoke("save_clients", { clients: state.clients.clients, ...rangeArgs() }));
  } catch (problem) {
    error.textContent = String(problem);
    error.hidden = false;
  }
}

function openClients(addNew) {
  clientsDialog.showModal();
  if (addNew && !state.clients.clients.length) {
    renderClientEditors();
    return;
  }
  renderClientEditors();
}

const blankClient = (name) => ({ name, own: false, keywords: name ? [name] : [], domains: [], apps: [], payments: [] });

$("clients-edit").addEventListener("click", () => openClients(false));
$("clients-body").addEventListener("click", (event) => { if (event.target.id === "clients-start") openClients(true); });
$("clients-close").addEventListener("click", () => clientsDialog.close());
$("client-add").addEventListener("click", () => {
  state.clients.clients.push(blankClient(""));
  renderClientEditors();
  const inputs = $("client-editors").querySelectorAll("input.name");
  inputs[inputs.length - 1].focus();
});
clientsDialog.addEventListener("click", (event) => {
  if (event.target === clientsDialog) return clientsDialog.close();
  const suggest = event.target.closest("[data-suggest]");
  if (suggest) {
    state.clients.clients.push(blankClient(suggest.dataset.suggest));
    return saveClients();
  }
  const delClient = event.target.closest("[data-del-client]");
  if (delClient) {
    state.clients.clients.splice(Number(delClient.dataset.delClient), 1);
    return saveClients();
  }
  const delPayment = event.target.closest("[data-del-payment]");
  if (delPayment) {
    const [i, j] = delPayment.dataset.delPayment.split(":").map(Number);
    state.clients.clients[i].payments.splice(j, 1);
    return saveClients();
  }
});
clientsDialog.addEventListener("change", (event) => {
  const field = event.target.dataset.f;
  const box = event.target.closest("[data-client]");
  if (!field || !box) return;
  const client = state.clients.clients[Number(box.dataset.client)];
  if (field === "own") client.own = event.target.checked;
  else if (field === "name") client.name = event.target.value.trim();
  else client[field] = list(event.target.value);
  if (client.name) saveClients();
});
clientsDialog.addEventListener("submit", (event) => {
  const form = event.target.closest("[data-pay]");
  if (!form) return;
  event.preventDefault();
  const data = new FormData(form);
  state.clients.clients[Number(form.dataset.pay)].payments.push({
    date: data.get("date"), amount: Number(data.get("amount")), currency: data.get("currency"), note: String(data.get("note") || "").trim(),
  });
  saveClients();
});

// ---------- Export ----------

let toastTimer;
function toast(text) {
  $("toast").textContent = text;
  $("toast").hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { $("toast").hidden = true; }, 6000);
}
$("export-btn").addEventListener("click", async () => {
  menu.hidden = true;
  try {
    toast(t.exported(await invoke("export_csv", { period: state.period, date: state.date })));
  } catch (problem) {
    toast(String(problem));
  }
});

// The current day or week keeps itself up to date while the window is open.
setInterval(() => {
  if (!document.hidden && state.data && state.data.is_current) load();
}, 30000);
document.addEventListener("visibilitychange", () => { if (!document.hidden) load(); });
if (tauri) tauri.event.listen("status-changed", load);

load();
