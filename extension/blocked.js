// The page shown in place of a site zman is blocking.

const params = new URLSearchParams(location.search);
const site = params.get("site") || "";
const reason = params.get("reason") || "site";
const until = Number(params.get("until")) || 0;
const limit = Number(params.get("limit")) || 0;
const target = params.get("url") || "";

const spanish = (navigator.language || "").toLowerCase().startsWith("es");
const clock = (ms) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
const t = spanish
  ? {
      title: { focus: "Estás en modo foco", schedule: "Horario de foco", limit: "Se acabó el tiempo de hoy", site: "Sitio bloqueado" },
      text: {
        focus: `${site} vuelve cuando termine la sesión, a las ${clock(until)}.`,
        schedule: `Los sitios que distraen están bloqueados hasta las ${clock(until)}.`,
        limit: `Ya usaste los ${limit} minutos de hoy. ${site} vuelve mañana.`,
        site: `Le pediste a zman que bloquee ${site}.`,
      },
      retry: "Probar de nuevo",
      footer: "Bloqueado por zman. Se cambia desde la app, en Foco.",
    }
  : {
      title: { focus: "You are in focus mode", schedule: "Focus hours", limit: "Today's time is up", site: "Site blocked" },
      text: {
        focus: `${site} comes back when the session ends, at ${clock(until)}.`,
        schedule: `Distracting sites are blocked until ${clock(until)}.`,
        limit: `You have used today's ${limit} minutes. ${site} comes back tomorrow.`,
        site: `You asked zman to block ${site}.`,
      },
      retry: "Try again",
      footer: "Blocked by zman. Change it in the app, under Focus.",
    };

const $ = (id) => document.getElementById(id);
document.title = `${t.title[reason] || t.title.site} · zman`;
$("title").textContent = t.title[reason] || t.title.site;
$("text").textContent = t.text[reason] || t.text.site;
$("retry").textContent = t.retry;
$("footer").textContent = t.footer;

// Only ever go back to a web page, never to a script or a local file.
const safe = /^https?:\/\//i.test(target);
$("retry").hidden = !safe;
$("retry").addEventListener("click", () => location.replace(target));

// Focus sessions and schedules count down, and the page returns by itself.
if ((reason === "focus" || reason === "schedule") && until > Date.now()) {
  const total = Math.max(until - Date.now(), 1);
  const circumference = 207.35;
  const tick = () => {
    const left = until - Date.now();
    if (left <= 0) {
      if (safe) location.replace(target);
      return;
    }
    const secs = Math.ceil(left / 1000);
    const hours = Math.floor(secs / 3600);
    const pad = (n) => String(n).padStart(2, "0");
    $("countdown").textContent = hours
      ? `${hours}:${pad(Math.floor(secs / 60) % 60)}:${pad(secs % 60)}`
      : `${Math.floor(secs / 60)}:${pad(secs % 60)}`;
    $("arc").style.strokeDashoffset = String(circumference * (1 - left / total));
  };
  $("countdown").hidden = false;
  tick();
  setInterval(tick, 1000);
}
