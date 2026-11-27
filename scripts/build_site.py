"""Builds the website into docs/, which GitHub Pages serves.

site/index.html holds both languages side by side, as pairs of
`<span lang="en">` and `<span lang="es">`. This script writes one page per
language (docs/index.html in English, docs/es/index.html in Spanish), each
with its own title, description, social card, structured data and links to
the other language, plus the blog (see blog.py), sitemap.xml and robots.txt.

The address of the site is https://malksof.github.io/zman/ unless the file
site/CNAME exists; then it is the domain written there (for example
zman.example.com), and GitHub Pages serves the site on that domain.

    python scripts/build_site.py

The dashboard demo under docs/demo is rebuilt separately by demo_site.py.
"""
import html, io, json, os, re, shutil

import blog

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "site")
OUT = os.path.join(ROOT, "docs")
URL = "https://malksof.github.io/zman/"
if os.path.exists(os.path.join(SRC, "CNAME")):
    URL = "https://%s/" % io.open(os.path.join(SRC, "CNAME"), encoding="utf-8").read().strip()
REPO = "https://github.com/MalkSof/zman"
VERSION = "0.4.0"
# The hosted service, sold as Tu Hora Vale. With CLOUD_URL empty the plan's
# button leads to the waitlist instead of the sign-up page.
CLOUD_URL = "https://app.tuhoravale.com"
PRICE_ARS = "9.900"
TRIAL_DAYS = 15
CLOUD = {
    "en": {"price": "ARS " + PRICE_ARS.replace(".", ","), "open": "Try it %d days free" % TRIAL_DAYS, "open_note": "No card needed. Cancel any time.",
           "closed": "Tell me when it opens", "closed_note": "Opening soon. %d days free when it does." % TRIAL_DAYS},
    "es": {"price": "$" + PRICE_ARS, "open": "Probar %d días gratis" % TRIAL_DAYS, "open_note": "Sin tarjeta. Cancelás cuando quieras.",
           "closed": "Avisame cuando abra", "closed_note": "Abre muy pronto, con %d días gratis." % TRIAL_DAYS},
}
# Where "Join the waitlist" leads. Put the address of a form here (one that
# collects an email) to replace the default, which opens a GitHub issue.
WAITLIST = {
    "cloud": REPO + "/issues/new?title=Waitlist%3A+zman+Cloud&body=I+want+to+know+when+zman+Cloud+opens.",
    "teams": REPO + "/issues/new?title=Waitlist%3A+zman+for+teams&body=I+want+to+know+when+zman+for+teams+opens.%0A%0AHow+many+people%3A+",
}

PAGES = {
    "en": {
        "path": "",
        "locale": "en_US",
        "title": "zman · Hours per client and your real hourly rate, for freelancers",
        "description": "Free automatic time tracker for freelancers. zman splits your hours by client without timers, "
                       "crosses them with what you get paid and shows your real rate per hour. Open source, for Windows.",
        "card_title": "Know what you earn per hour with each client.",
        "faq_title": "Questions people ask",
        "posts_title": "From the blog",
        "posts_more": "All articles",
        "faq": [
            ("Is zman free?",
             "The app is free and open source under the MIT license, with no account needed: you download it and it works, including hours per client and your rate per hour. Tu Hora Vale is our optional paid service on top of it: it adds a web panel, your phone and several computers."),
            ("How does it know which client I am working for?",
             "You add each client once, with a word that shows up in the window title (usually the project folder your editor or terminal displays), the client's website or an app. From then on zman assigns the time by itself, and it suggests the project folders it saw that have no client yet. You can change the rules at any time."),
            ("How does it compute my rate per hour?",
             "You enter what each client paid, in pesos, dollars or euros, or you tell your AI assistant and it saves it. zman divides what was paid in the month by the hours it tracked for that client. Your own projects count hours but have no rate."),
            ("Is it an alternative to RescueTime?",
             "Yes. Like RescueTime, zman tracks the apps and websites you use automatically and gives you a productivity score, categories and focus time. The difference is that zman is open source and keeps your data in a file on your own computer instead of in someone else's cloud."),
            ("How does it work with Claude and ChatGPT?",
             "zman speaks MCP, the protocol AI assistants use to reach tools. Press Connect in the app and Claude Desktop can answer questions like \"where did my week go?\" from your real numbers. With your own zman server, ChatGPT and Claude on the web connect too. Assistants only receive totals, never window titles."),
            ("Can it block distracting websites?",
             "Yes. Start a focus session to block entertainment and social media for 25, 50 or 90 minutes, set daily limits such as \"YouTube, 30 minutes a day\", or block sites during your working hours. Blocking uses the zman browser extension."),
            ("What does zman record? Is it private?",
             "Only the name of the app in use, the window title, the address of the site in your browser and for how long. It never records keystrokes, screenshots or the content of pages, and it skips private browser windows. Nothing leaves your computer unless you sign in to a server you run yourself."),
            ("Which systems does it support?",
             "Windows 10 and 11 today, with an Android companion app for phone usage. macOS and Linux are not supported yet."),
        ],
    },
    "es": {
        "path": "es/",
        "locale": "es_AR",
        "title": "zman · Horas por cliente y tu tarifa real por hora, para freelancers",
        "description": "Time tracker automático y gratis para freelancers. zman separa tus horas por cliente sin cronómetros, "
                       "las cruza con lo que cobrás y te muestra tu tarifa real por hora. Open source, para Windows.",
        "card_title": "Sabé cuánto ganás por hora con cada cliente.",
        "faq_title": "Preguntas frecuentes",
        "posts_title": "Del blog",
        "posts_more": "Todos los artículos",
        "faq": [
            ("¿zman es gratis?",
             "La app es gratis y open source, con licencia MIT, y no pide cuenta: la descargás y funciona, con las horas por cliente y tu tarifa por hora incluidas. Tu Hora Vale es nuestro servicio pago opcional arriba de la app: suma el panel web, el celular y varias computadoras."),
            ("¿Cómo sabe para qué cliente estoy trabajando?",
             "Cargás cada cliente una sola vez, con una palabra que aparece en el título de la ventana (casi siempre la carpeta del proyecto que muestran tu editor o tu terminal), el sitio del cliente o una app. Desde ahí zman asigna el tiempo solo, y te sugiere las carpetas de proyecto que vio y todavía no tienen cliente. Las reglas se pueden cambiar cuando quieras."),
            ("¿Cómo calcula mi tarifa por hora?",
             "Cargás lo que te pagó cada cliente, en pesos, dólares o euros, o se lo decís a tu asistente de IA y lo anota. zman divide lo cobrado en el mes por las horas que registró para ese cliente. Tus proyectos propios cuentan horas pero no tienen tarifa."),
            ("¿Puedo facturar en ARCA desde zman?",
             "Todavía no. Estamos trabajando para que Tu Hora Vale arme la factura de ARCA a partir de las horas del mes. Hoy zman mide las horas y calcula tu tarifa; la factura la seguís haciendo donde la hacés siempre."),
            ("¿Es una alternativa a RescueTime?",
             "Sí. Igual que RescueTime, zman registra solo las apps y los sitios que usás y te da un puntaje de productividad, categorías y tiempo de foco. La diferencia es que zman es open source y guarda tus datos en un archivo en tu propia computadora, no en la nube de otro."),
            ("¿Cómo funciona con Claude y ChatGPT?",
             "zman habla MCP, el protocolo que usan los asistentes de IA para conectarse a herramientas. Tocás Conectar en la app y Claude Desktop puede responder preguntas como \"¿en qué se me fue la semana?\" con tus números reales. Con tu propio servidor zman también se conectan ChatGPT y Claude en la web. Los asistentes solo reciben totales, nunca títulos de ventanas."),
            ("¿Puede bloquear sitios que distraen?",
             "Sí. Iniciá una sesión de foco para bloquear entretenimiento y redes sociales por 25, 50 o 90 minutos, poné límites diarios como \"YouTube, 30 minutos por día\", o bloqueá sitios en tu horario de trabajo. El bloqueo usa la extensión de navegador de zman."),
            ("¿Qué registra zman? ¿Es privado?",
             "Solo el nombre de la app en uso, el título de la ventana, la dirección del sitio en tu navegador y por cuánto tiempo. Nunca registra teclas, capturas de pantalla ni el contenido de las páginas, y saltea las ventanas privadas del navegador. Nada sale de tu computadora salvo que inicies sesión en un servidor tuyo."),
            ("¿En qué sistemas funciona?",
             "Hoy en Windows 10 y 11, con una app Android complementaria para el uso del teléfono. macOS y Linux todavía no están soportados."),
        ],
    },
}

ALL_LANGS = tuple(PAGES)
BRAND = "zman"
# Exact pieces of text to change on every page written, and sections to drop.
REPLACE = []
DROP = []
# A pattern for the name of the app where the site calls it something else.
RENAME = None
# Where the Spanish home page really lives, when it is not this site.
HOME_CANONICAL = {"es": "https://tuhoravale.com/"}

# `SITE=tuhoravale python scripts/build_site.py` builds, from these same
# sources, the site of the hosted product into dist/tuhoravale: Spanish
# only, under its own name, with the posts written for freelancers.
if os.environ.get("SITE") == "tuhoravale":
    BRAND = "Tu Hora Vale"
    URL = "https://tuhoravale.com/"
    OUT = os.path.join(ROOT, "dist", "tuhoravale")
    HOME_CANONICAL = {}
    page = PAGES["es"]
    page.update(
        path="",
        title="Tu Hora Vale · Sabé cuánto ganás por hora con cada cliente",
        description="Para freelancers: Tu Hora Vale separa tus horas por cliente sin cronómetros, las cruza con lo que cobrás "
                    "y te muestra tu tarifa real por hora. App gratis para Windows y panel web con %d días de prueba." % TRIAL_DAYS,
        card_title="Sabé cuánto ganás por hora con cada cliente.",
    )
    keep = {q: a for q, a in page["faq"]}
    page["faq"] = [
        ("¿Tu Hora Vale es gratis?",
         "La app de escritorio es gratis y open source: la descargás y funciona sin cuenta, con las horas por cliente "
         "y tu tarifa por hora incluidas. El plan Freelancer suma el panel web, el celular y varias computadoras: cuesta $%s por mes, "
         "con %d días de prueba sin tarjeta." % (PRICE_ARS, TRIAL_DAYS)),
        ("¿Cómo sabe para qué cliente estoy trabajando?", keep["¿Cómo sabe para qué cliente estoy trabajando?"]),
        ("¿Cómo calcula mi tarifa por hora?", keep["¿Cómo calcula mi tarifa por hora?"]),
        ("¿Puedo facturar en ARCA desde Tu Hora Vale?",
         "Todavía no. Estamos trabajando para que arme la factura de ARCA a partir de las horas del mes. Hoy mide las horas y calcula "
         "tu tarifa; la factura la seguís haciendo donde la hacés siempre."),
        ("¿Qué registra la app? ¿Es privado?", keep["¿Qué registra zman? ¿Es privado?"]),
        ("¿Cómo funciona con Claude y ChatGPT?",
         "La app habla MCP, el protocolo que usan los asistentes de IA para conectarse a herramientas. Tocás Conectar y Claude Desktop puede "
         "responder preguntas como \"¿cuánto gané por hora con este cliente?\" con tus números reales. Con el plan Freelancer también se conectan "
         "ChatGPT y Claude en la web. Los asistentes solo reciben totales, nunca títulos de ventanas."),
        ("¿En qué sistemas funciona?", keep["¿En qué sistemas funciona?"]),
    ]
    PAGES = {"es": page}
    blog.only("es", URL)
    REPLACE = [
        ('alt="zman" width="28" height="28">zman</a>', 'alt="Tu Hora Vale" width="28" height="28">Tu Hora Vale</a>'),
        ('<meta property="og:site_name" content="zman">', '<meta property="og:site_name" content="Tu Hora Vale">'),
        ('"name": "zman"', '"name": "Tu Hora Vale"'),
        ('title="zman" href=', 'title="Tu Hora Vale" href='),
        ("<span>zman · ", "<span>Tu Hora Vale · "),
        ("Blog de zman · registro de tiempo, foco e IA", "Blog de Tu Hora Vale · horas, clientes y cuánto cobrar"),
        ("Datos, guías y comparativas sobre tiempo, foco y trabajo con IA.", "Guías para freelancers: cuánto cobrar, qué cliente conviene y cómo medir tus horas."),
        ("zman registra solo en qué trabajás, lo separa por cliente", "Tu Hora Vale registra solo en qué trabajás, lo separa por cliente"),
        ("Tu Hora Vale, nuestro servicio en la nube, le suma la web", "El plan Freelancer le suma la web"),
        ("<h2>Instalalo en un minuto</h2>", "<h2>Instalá Tu Hora Vale en un minuto</h2>"),
        ("Levantá tu propio servidor zman", "Levantá tu propio servidor, que es open source,"),
    ]
    # The app is called Tu Hora Vale; zman is only the name of its source
    # code. What stays: addresses, and the commands of the terminal tool.
    RENAME = re.compile(r"(?<![\w/.@+%-])zman(?![\w/+-])(?!\.\w)(?! (?:today|week|export|status|daemon)\b)")
    # Services to order for companies and families belong to the other site.
    DROP = [r'\n    <section id="services" class="services">.*?</section>\n']
used = {old: 0 for old, _ in REPLACE}


def one_language(text, lang):
    """Keeps the spans of `lang` (unwrapped) and drops the other language's."""
    for other in ALL_LANGS:
        if other != lang:
            text = re.sub(r'<span lang="%s">.*?</span>' % other, "", text, flags=re.S)
    return re.sub(r'<span lang="%s">(.*?)</span>' % lang, r"\1", text, flags=re.S)


def head(lang):
    page = PAGES[lang]
    esc = lambda value: html.escape(value, quote=True)
    url = URL + page["path"]
    image = "%sog-%s.png" % (URL, lang)
    structured = [
        {
            "@context": "https://schema.org",
            "@type": "SoftwareApplication",
            "name": BRAND,
            "description": page["description"],
            "url": url,
            "image": image,
            "applicationCategory": "ProductivityApplication",
            "operatingSystem": "Windows 10, Windows 11",
            "softwareVersion": VERSION,
            "downloadUrl": REPO + "/releases/latest",
            "license": "https://opensource.org/licenses/MIT",
            "isAccessibleForFree": True,
            "inLanguage": lang,
            "offers": {"@type": "Offer", "price": "0", "priceCurrency": "USD"},
            "codeRepository": REPO,
        },
        {
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "inLanguage": lang,
            "mainEntity": [
                {"@type": "Question", "name": q, "acceptedAnswer": {"@type": "Answer", "text": a}}
                for q, a in page["faq"]
            ],
        },
    ]
    lines = [
        "<title>%s</title>" % esc(page["title"]),
        '<meta name="description" content="%s">' % esc(page["description"]),
        '<link rel="canonical" href="%s">' % HOME_CANONICAL.get(lang, url),
    ]
    lines += ['<link rel="alternate" hreflang="%s" href="%s">' % (code, URL + other["path"]) for code, other in PAGES.items()]
    lines += [
        '<link rel="alternate" hreflang="x-default" href="%s">' % URL,
        '<meta property="og:type" content="website">',
        '<meta property="og:site_name" content="zman">',
        '<meta property="og:url" content="%s">' % url,
        '<meta property="og:title" content="%s">' % esc(page["card_title"]),
        '<meta property="og:description" content="%s">' % esc(page["description"]),
        '<meta property="og:image" content="%s">' % image,
        '<meta property="og:image:width" content="1200">',
        '<meta property="og:image:height" content="630">',
        '<meta property="og:locale" content="%s">' % page["locale"],
        '<meta name="twitter:card" content="summary_large_image">',
        '<meta name="twitter:title" content="%s">' % esc(page["card_title"]),
        '<meta name="twitter:description" content="%s">' % esc(page["description"]),
        '<meta name="twitter:image" content="%s">' % image,
        '<meta name="theme-color" content="#14161c">',
    ]
    lines += ['<script type="application/ld+json">%s</script>' % json.dumps(item, ensure_ascii=False) for item in structured]
    return "\n".join("  " + line for line in lines)


def faq(lang):
    page = PAGES[lang]
    items = "\n".join(
        "        <details><summary>%s</summary><p>%s</p></details>" % (html.escape(q), html.escape(a)) for q, a in page["faq"]
    )
    return '    <section id="faq" class="faq">\n      <h2>%s</h2>\n      <div class="faq-list">\n%s\n      </div>\n    </section>' % (
        html.escape(page["faq_title"]), items)


def write(path, text):
    if path.endswith((".html", ".xml")):
        for old, new in REPLACE:
            used[old] += text.count(old)
            text = text.replace(old, new)
        for pattern in DROP:
            text = re.sub(pattern, "\n", text, flags=re.S)
        if RENAME:
            text = RENAME.sub(BRAND, text)
        if len(PAGES) == 1:
            # One language: nothing to switch to.
            text = re.sub(r'<span class="lang-toggle".*?</span>', "", text, flags=re.S)
            text = re.sub(r"\n  <script>\s*// Each language has its own page.*?</script>", "", text, flags=re.S)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with io.open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


if OUT.endswith("tuhoravale"):
    shutil.rmtree(OUT, ignore_errors=True)
posts, blog_paths = blog.build(SRC, OUT, URL, write)


def latest(lang, root):
    page = PAGES[lang]
    return ('    <section class="from-blog">\n      <h2>%s</h2>\n      <div class="post-list three">\n%s\n      </div>\n'
            '      <div class="cta center"><a class="btn ghost" href="%s%sblog/">%s</a></div>\n    </section>') % (
        page["posts_title"], blog.cards(posts, lang, root, 3), root, page["path"], page["posts_more"])


template = io.open(os.path.join(SRC, "index.html"), encoding="utf-8").read()
for lang, page in PAGES.items():
    root = "../" * page["path"].count("/")
    text = one_language(template, lang)
    values = {
        "lang": lang,
        "root": root,
        "head": head(lang),
        "faq": faq(lang),
        "posts": latest(lang, root),
        "price": CLOUD[lang]["price"],
        "cloud_link": html.escape(CLOUD_URL.rstrip("/") + "/signup" if CLOUD_URL else WAITLIST["cloud"], quote=True),
        "cloud_label": CLOUD[lang]["open" if CLOUD_URL else "closed"],
        "cloud_note": CLOUD[lang]["open_note" if CLOUD_URL else "closed_note"],
        "waitlist_teams": html.escape(WAITLIST["teams"], quote=True),
        "blog": root + page["path"] + "blog/",
        "en": root or "./",
        "es": root + PAGES["es"]["path"],
    }
    for key, value in values.items():
        text = text.replace("{{%s}}" % key, value)
    assert "{{" not in text and 'lang="en">' not in text.split("</head>")[1].replace('hreflang="en">', ""), lang
    write(os.path.join(OUT, page["path"], "index.html"), text)

other_site = BRAND != "zman"
skip = {"index.html", "CNAME", "og-en.png", "og-es.png"} if other_site else {"index.html", "og-tuhoravale.png"}
for name in os.listdir(SRC):
    if name not in skip and os.path.isfile(os.path.join(SRC, name)):
        shutil.copyfile(os.path.join(SRC, name), os.path.join(OUT, "og-es.png" if name == "og-tuhoravale.png" else name))
if other_site:
    # The live demo of the dashboard, built by demo_site.py into docs/demo.
    shutil.copytree(os.path.join(ROOT, "docs", "demo"), os.path.join(OUT, "demo"), dirs_exist_ok=True)

write(os.path.join(OUT, "sitemap.xml"), '<?xml version="1.0" encoding="UTF-8"?>\n'
      '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n'
      + "".join(
          "  <url>\n    <loc>%s</loc>\n%s  </url>\n" % (
              URL + page["path"],
              "".join('    <xhtml:link rel="alternate" hreflang="%s" href="%s"/>\n' % (code, URL + other["path"])
                      for code, other in PAGES.items()))
          for page in PAGES.values())
      + "".join(
          "  <url>\n    <loc>%s</loc>\n%s  </url>\n" % (
              URL + path,
              "".join('    <xhtml:link rel="alternate" hreflang="%s" href="%s"/>\n' % (code, URL + other)
                      for code, other in others.items()))
          for path, others in blog_paths)
      + "</urlset>\n")
# Only read by crawlers when the site sits at the root of its domain.
write(os.path.join(OUT, "robots.txt"), "User-agent: *\nAllow: /\n\nSitemap: %ssitemap.xml\n" % URL)
write(os.path.join(OUT, ".nojekyll"), "")
unused = [old for old, count in used.items() if not count]
assert not unused, "text to replace was not found: %r" % unused
print("site built:", ", ".join(URL + page["path"] for page in PAGES.values()))
