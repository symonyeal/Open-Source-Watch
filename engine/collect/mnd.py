"""Taiwan Ministry of National Defense daily reports of PLA activity around
Taiwan (docs/SOURCES.md).

The English list (`/en/news/plaactlist/<page>`, ten reports a page) links one
report a day. A report states its window, "6 a.m. Oct. 28 (Wed.) to 6 a.m.
Oct. 29 (Thu.) (UTC+8)", and counts: sorties of PLA aircraft, PLAN ships,
official ships, balloons, and how many of the sorties entered the ADIZ or
crossed the median line. One raw report is one MND report, its native id the
report number. These are a party's own claims: graded in region.toml, shown as
a dated series, never fused into events. A report without a window (not a daily
report) is skipped; a page on which no report parses fails the poll.
"""
import html as entities
import json
import re
from datetime import UTC, datetime, timedelta
from types import SimpleNamespace

from engine import store
from engine.collect import EXCERPT
from engine.collect import fetch as get

SITE = "https://www.mnd.gov.tw"
LIST = SITE + "/en/news/plaactlist"
PAGES = 9          # backfill depth, about 90 days
CADENCE = 43200
MONTHS = {m: k for k, m in enumerate(
    ("jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"), 1)}
WINDOW = re.compile(r"(?s)6\s*a\.m\.\s*([A-Za-z]{3})[a-z]*\.?\s*(\d{1,2})\b.*?"
                    r"to\s*6\s*a\.m\.\s*([A-Za-z]{3})[a-z]*\.?\s*(\d{1,2})\b.*?\(UTC\+8\)")
COUNTS = {   # field: (pattern of n, pattern of an explicit none)
    "aircraft": (r"(\d+)\s+sorties?\s+of\s+PLA\s+aircraft", r"\bno\s+PLA\s+aircraft"),
    "ships": (r"(\d+)\s+PLAN\s+(?:ships?|vessels?)", r"\bno\s+PLAN\s+(?:ships?|vessels?)"),
    "official": (r"(\d+)\s+official\s+ships?", r"\bno\s+official\s+ships?"),
    "balloons": (r"(\d+)\s+(?:PRC\s+)?balloons?", r"\bno\s+(?:PRC\s+)?balloons?"),
}


def sources(R):
    return [{"id": "mnd:Taiwan MND", "region": "taiwan-strait", "url": LIST, "cadence": CADENCE}]


def links(page):
    """[(report number, publication day)] of one list page."""
    return [(n, d.replace(".", "-")) for n, d in re.findall(
        r'(?i)href="/en/News/PLAAct/(\d+)"[^>]*>\s*<div class="date[^"]*">\s*(\d{4}\.\d{2}\.\d{2})',
        page)]


def fetch(src):
    """{report number: page html} of the reports not yet stored, from list pages
    newest first; stops at the first page with a stored report, or after PAGES."""
    have = {x["n"] for x in store.query(
        "SELECT DISTINCT json_extract_string(payload, '$.id') n FROM raw_report "
        "WHERE source_id LIKE 'mnd:%'")}
    out = {}
    for k in range(1, PAGES + 1):
        L = links(get({"url": LIST if k == 1 else f"{LIST}/{k}"}).text)
        new = [n for n, _ in L if n not in have]
        out |= {n: get({"url": f"{SITE}/en/News/PLAAct/{n}"}).text for n in new}
        if len(new) < len(L) or not L:
            break
    return SimpleNamespace(content=json.dumps(out).encode())


def text(page):
    """Plain text of the report body (the 'maincontent' block up to its keywords)."""
    i = page.find("maincontent")
    if i < 0:
        raise ValueError("MND report body missing")
    s = entities.unescape(re.sub(r"<[^>]+>", " ", page[i:]))
    return re.sub(r"\s+", " ", s.split("Keywords")[0]).strip()


def local6(month, day, published):
    """06:00 UTC+8 of the stated month and day, in UTC; the year is the
    publication year, moved by one across a December/January boundary."""
    m, y = MONTHS[month.lower()], published.year
    y += (m == 1 and published.month == 12) - (m == 12 and published.month == 1)
    return datetime(y, m, int(day), 6, tzinfo=UTC) - timedelta(hours=8)


def report(n, page):
    """Payload of report n, or None if the page states no reporting window."""
    pub = re.search(r"(?s)pageinfo.*?(\d{4})\.(\d{2})\.(\d{2})", page)
    if not pub:
        raise ValueError("MND publication date missing")
    published = datetime(*map(int, pub.groups()), tzinfo=UTC)
    s = text(page)
    w = WINDOW.search(s)
    if not w:
        return None
    start, end = local6(*w.group(1, 2), published), local6(*w.group(3, 4), published)
    if not timedelta(0) < end - start <= timedelta(days=3):
        raise ValueError("MND reporting window invalid")
    c = {k: int(m.group(1)) if (m := re.search("(?i)" + p, s)) else
         0 if re.search("(?i)" + z, s) else None for k, (p, z) in COUNTS.items()}
    moved = re.search(r"(?i)(\d+)\s+out\s+of\s+\d+\s+sorties?\s+([^.]*)", s)
    clause = moved.group(2) if moved else ""
    c["adiz"] = int(moved.group(1)) if moved and "ADIZ" in clause else None
    c["median_line"] = int(moved.group(1)) if moved and re.search(r"(?i)median\s+line",
                                                                   clause) else None
    if all(v is None for v in c.values()):
        raise ValueError("MND report states no counts")
    return {"id": n, "published": published.date().isoformat(), "start": start.isoformat(),
            "end": end.isoformat(), "day": (end + timedelta(hours=8)).date().isoformat(),
            **c, "text": s[w.start():][:EXCERPT]}


def parse(body, src):
    P = json.loads(body)
    items = [{"native_id": n, "published_at": datetime.fromisoformat(p["end"]),
              "url": f"{SITE}/en/News/PLAAct/{n}", "payload": p}
             for n, page in P.items() if (p := report(n, page))]
    if P and not items:
        raise ValueError("No MND report on the page parsed")
    return items
