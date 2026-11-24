"""Draws the images shown when the website is shared (site/og-en.png, og-es.png).

    python scripts/social_card.py        (needs Pillow and the Segoe UI fonts)
"""
import os
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTS = "C:/Windows/Fonts/"
W, H = 1200, 630
BG, INK, MUTED, ACCENT = "#14161c", "#f0efec", "#a3a29b", "#3987e5"
COLORS = ["#3987e5", "#d95926", "#199e70", "#c98500", "#e66767"]

TEXT = {
    "en": (["Know where your", "time goes."], "Then ask Claude about it.",
           "Free · open source · your data stays on your computer"),
    "es": (["Sabé a dónde se", "va tu tiempo."], "Y preguntáselo a Claude.",
           "Gratis · open source · tus datos quedan en tu computadora"),
}
# Hours of a made-up day: (development, work, communication, browsing, fun) in minutes.
DAY = [(30, 0, 0, 0, 0), (34, 4, 3, 8, 0), (44, 9, 0, 0, 0), (35, 14, 0, 0, 0), (2, 0, 0, 0, 0),
       (36, 0, 17, 0, 0), (16, 18, 12, 5, 9), (32, 6, 0, 0, 15), (22, 33, 0, 5, 0), (12, 0, 7, 0, 3)]


def font(name, size):
    return ImageFont.truetype(FONTS + name, size)


for lang, (headline, accent, note) in TEXT.items():
    image = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(image)

    icon = Image.open(os.path.join(ROOT, "site", "icon.png")).convert("RGBA").resize((64, 64), Image.LANCZOS)
    image.paste(icon, (72, 64), icon)
    draw.text((152, 66), "zman", font=font("seguisb.ttf", 44), fill=INK)

    y = 190
    for line in headline:
        draw.text((72, y), line, font=font("segoeuib.ttf", 68), fill=INK)
        y += 82
    draw.text((72, y + 4), accent, font=font("segoeuib.ttf", 52), fill=ACCENT)
    draw.text((72, 540), note, font=font("segoeui.ttf", 26), fill=MUTED)

    # A small stacked column chart, like the app's "activity by hour".
    left, bottom, top, step, bar = 760, 500, 150, 38, 24
    panel = (left - 36, top - 40, W - 56, bottom + 44)
    draw.rounded_rectangle(panel, radius=24, fill="#1c1f27", outline="#2b2f3a")
    for i in range(1, 4):
        line_y = bottom - i * (bottom - top) / 4
        draw.line((left - 8, line_y, W - 84, line_y), fill="#2b2f3a", width=1)
    for i, parts in enumerate(DAY):
        x, base = left + i * step, bottom
        for minutes, color in zip(parts, COLORS):
            if minutes:
                height = minutes / 60 * (bottom - top)
                draw.rectangle((x, base - height, x + bar, base), fill=color)
                base -= height + 2
    image.save(os.path.join(ROOT, "site", "og-%s.png" % lang), optimize=True)
print("social cards drawn")
