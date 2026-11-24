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
VERSION = "0.2.0"

PAGES = {
    "en": {
        "path": "",
        "locale": "en_US",
        "title": "zman · Free automatic time tracker, a RescueTime alternative",
        "description": "Free, open source automatic time tracker for Windows. See where your time goes, "
                       "block distracting sites and ask Claude or ChatGPT about your day. Your data stays on your computer.",
        "card_title": "Know where your time goes. Then ask Claude about it.",
        "faq_title": "Questions people ask",
        "posts_title": "From the blog",
        "posts_more": "All articles",
        "faq": [
            ("Is zman free?",
             "Yes. zman is free and open source under the MIT license. There is no account, no trial and no paid plan: you download it and it works."),
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
        "title": "zman · Time tracker automático y gratis, alternativa a RescueTime",
        "description": "Time tracker automático, gratis y open source para Windows. Mirá en qué se te va el tiempo, "
                       "bloqueá los sitios que distraen y preguntale a Claude o ChatGPT por tu día. Tus datos quedan en tu computadora.",
        "card_title": "Sabé a dónde se va tu tiempo. Y preguntáselo a Claude.",
        "faq_title": "Preguntas frecuentes",
        "posts_title": "Del blog",
        "posts_more": "Todos los artículos",
        "faq": [
            ("¿zman es gratis?",
             "Sí. zman es gratis y open source, con licencia MIT. No hay cuenta, ni prueba, ni plan pago: lo descargás y funciona."),
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


def one_language(text, lang):
    """Keeps the spans of `lang` (unwrapped) and drops the other language's."""
    for other in PAGES:
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
            "name": "zman",
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
        '<link rel="canonical" href="%s">' % url,
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
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with io.open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


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
        "blog": root + page["path"] + "blog/",
        "en": root or "./",
        "es": root + PAGES["es"]["path"],
    }
    for key, value in values.items():
        text = text.replace("{{%s}}" % key, value)
    assert "{{" not in text and 'lang="en">' not in text.split("</head>")[1].replace('hreflang="en">', ""), lang
    write(os.path.join(OUT, page["path"], "index.html"), text)

for name in os.listdir(SRC):
    if name != "index.html" and os.path.isfile(os.path.join(SRC, name)):
        shutil.copyfile(os.path.join(SRC, name), os.path.join(OUT, name))

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
print("site built:", ", ".join(URL + page["path"] for page in PAGES.values()))
