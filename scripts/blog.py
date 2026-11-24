"""The blog of the website. Used by build_site.py.

Posts live in site/blog as `<name>.<lang>.html`: a few `key: value` lines
(title, description, slug, date), a line with `---`, and the article as
HTML. The two languages of a post share `<name>`, which is how each page
links to its translation.
"""
import html, io, json, os

REPO = "https://github.com/MalkSof/zman"
TEXT = {
    "en": {"prefix": "", "blog": "Blog", "lead": "Numbers, guides and comparisons about time, focus and working with AI.",
           "title": "zman blog · time tracking, focus and AI", "read": "Read", "home": "Home",
           "cta_title": "See where your own time goes", "cta_text": "zman is a free, open source automatic time tracker for Windows. Your data stays on your computer.",
           "cta": "Download zman free", "back": "All articles", "footer": "free and open source time tracker",
           "date": lambda d: "%s %d, %s" % (["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][int(d[5:7]) - 1], int(d[8:10]), d[:4])},
    "es": {"prefix": "es/", "blog": "Blog", "lead": "Datos, guías y comparativas sobre tiempo, foco y trabajo con IA.",
           "title": "Blog de zman · registro de tiempo, foco e IA", "read": "Leer", "home": "Inicio",
           "cta_title": "Mirá a dónde se va tu propio tiempo", "cta_text": "zman es un time tracker automático, gratis y open source para Windows. Tus datos quedan en tu computadora.",
           "cta": "Descargar zman gratis", "back": "Todos los artículos", "footer": "time tracker gratis y open source",
           "date": lambda d: "%d de %s de %s" % (int(d[8:10]), ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"][int(d[5:7]) - 1], d[:4])},
}

PAGE = """<!doctype html>
<html lang="{lang}" data-lang="{lang}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{description}">
  <link rel="canonical" href="{url}">
{alternates}
  <meta property="og:type" content="{og_type}">
  <meta property="og:site_name" content="zman">
  <meta property="og:url" content="{url}">
  <meta property="og:title" content="{title}">
  <meta property="og:description" content="{description}">
  <meta property="og:image" content="{site}og-{lang}.png">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="theme-color" content="#14161c">
{structured}
  <link rel="alternate" type="application/rss+xml" title="zman" href="{site}{prefix}blog/feed.xml">
  <link rel="icon" href="{root}icon.png">
  <link rel="stylesheet" href="{root}site.css">
</head>
<body>
  <header class="nav">
    <a class="brand" href="{home}"><img src="{root}icon.png" alt="zman" width="28" height="28">zman</a>
    <nav>
      <a href="{home}">{home_label}</a>
      <a href="{blog}">Blog</a>
    </nav>
    <div class="nav-right">
      <span class="lang-toggle">{toggle}</span>
      <a class="btn small" href="%s">GitHub</a>
    </div>
  </header>
  <main>
{body}
    <section class="post-cta">
      <h2>{cta_title}</h2>
      <p>{cta_text}</p>
      <a class="btn" href="{home}#download">{cta}</a>
    </section>
  </main>
  <footer>
    <span>zman · {footer} · MIT</span>
    <a href="%s">github.com/MalkSof/zman</a>
  </footer>
</body>
</html>
""" % (REPO, REPO)


def load(src):
    """All posts as {name: {lang: post}}."""
    posts = {}
    folder = os.path.join(src, "blog")
    for file in sorted(os.listdir(folder)):
        name, lang, _ = file.rsplit(".", 2)
        header, body = io.open(os.path.join(folder, file), encoding="utf-8").read().split("\n---\n", 1)
        post = dict(line.split(": ", 1) for line in header.strip().splitlines())
        post.update(body=body.strip(), lang=lang, name=name, path="%sblog/%s/" % (TEXT[lang]["prefix"], post["slug"]))
        posts.setdefault(name, {})[lang] = post
    return posts


def render(lang, path, site, title, description, body, others, og_type="website", structured=None):
    """One page of the blog. `others` maps each language to the path of this page in it."""
    t = TEXT[lang]
    root = "../" * path.count("/")
    esc = lambda value: html.escape(value, quote=True)
    alternates = ['  <link rel="alternate" hreflang="%s" href="%s%s">' % (code, site, other) for code, other in others.items()]
    alternates.append('  <link rel="alternate" hreflang="x-default" href="%s%s">' % (site, others["en"]))
    toggle = "".join(
        '<a data-code="%s" hreflang="%s" href="%s%s">%s</a>' % (code, code, root, other, code.upper()) for code, other in others.items()
    )
    return PAGE.format(
        lang=lang, title=esc(title), description=esc(description), url=site + path, site=site, root=root,
        prefix=t["prefix"], alternates="\n".join(alternates), og_type=og_type,
        structured="" if structured is None else '  <script type="application/ld+json">%s</script>' % json.dumps(structured, ensure_ascii=False),
        home=root + t["prefix"] if root + t["prefix"] else "./", home_label=t["home"], blog=root + t["prefix"] + "blog/",
        toggle=toggle, body=body, cta_title=t["cta_title"], cta_text=t["cta_text"], cta=t["cta"], footer=t["footer"],
    )


def cards(posts, lang, root, limit=None):
    """Links to the posts of one language, newest first."""
    listed = sorted((p[lang] for p in posts.values() if lang in p), key=lambda p: p["date"], reverse=True)[:limit]
    return "\n".join(
        '        <a class="post-card" href="%s%s"><time datetime="%s">%s</time><h3>%s</h3><p>%s</p></a>'
        % (root, p["path"], p["date"], TEXT[lang]["date"](p["date"]), html.escape(p["title"]), html.escape(p["description"]))
        for p in listed
    )


def build(src, out, site, write):
    """Writes every blog page and feed. Returns the posts and the paths written, for the sitemap."""
    posts = load(src)
    paths = []
    for lang, t in TEXT.items():
        index = t["prefix"] + "blog/"
        body = '    <section class="post-index">\n      <h1>%s</h1>\n      <p class="section-lead">%s</p>\n      <div class="post-list">\n%s\n      </div>\n    </section>' % (
            t["blog"], t["lead"], cards(posts, lang, "../" * index.count("/")))
        others = {code: other["prefix"] + "blog/" for code, other in TEXT.items()}
        write(os.path.join(out, index, "index.html"), render(lang, index, site, t["title"], t["lead"], body, others))
        paths.append((index, others))

        items = []
        for versions in posts.values():
            if lang not in versions:
                continue
            post = versions[lang]
            others = {code: version["path"] for code, version in versions.items()}
            home = "../" * post["path"].count("/") + t["prefix"]
            article = post["body"].replace("{{home}}", home or "./")
            body = '    <article class="post">\n      <a class="post-back" href="../">← %s</a>\n      <h1>%s</h1>\n      <p class="post-meta"><time datetime="%s">%s</time></p>\n%s\n    </article>' % (
                t["back"], html.escape(post["title"]), post["date"], t["date"](post["date"]), article)
            structured = {
                "@context": "https://schema.org", "@type": "BlogPosting", "headline": post["title"],
                "description": post["description"], "datePublished": post["date"], "dateModified": post["date"],
                "inLanguage": lang, "mainEntityOfPage": site + post["path"], "image": "%sog-%s.png" % (site, lang),
                "author": {"@type": "Organization", "name": "zman", "url": site},
                "publisher": {"@type": "Organization", "name": "zman", "logo": {"@type": "ImageObject", "url": site + "icon.png"}},
            }
            write(os.path.join(out, post["path"], "index.html"),
                  render(lang, post["path"], site, post["title"], post["description"], body, others, "article", structured))
            paths.append((post["path"], others))
            items.append((post["date"], "    <item><title>%s</title><link>%s</link><guid>%s</guid><description>%s</description><pubDate>%s</pubDate></item>" % (
                html.escape(post["title"]), site + post["path"], site + post["path"], html.escape(post["description"]), post["date"])))
        feed = '<?xml version="1.0" encoding="UTF-8"?>\n<rss version="2.0">\n  <channel>\n    <title>%s</title>\n    <link>%s</link>\n    <description>%s</description>\n    <language>%s</language>\n%s\n  </channel>\n</rss>\n' % (
            html.escape(t["title"]), site + index, html.escape(t["lead"]), lang, "\n".join(item for _, item in sorted(items, reverse=True)))
        write(os.path.join(out, index, "feed.xml"), feed)
    return posts, paths
