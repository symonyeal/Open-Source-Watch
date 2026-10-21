"""RSS 2.0 and Atom feeds: the conflict's newsFeeds (upstream src/app/api/news/route.ts)."""
from collections import Counter
from datetime import UTC, datetime
from email.utils import parsedate_to_datetime
from functools import partial
from xml.etree import ElementTree as ET

from engine.collect import EXCERPT, get

CADENCE = 300
PER_FEED = 15  # upstream keeps the first 15 items of a feed


def feeds(r):
    """{source_id: feed}; feeds sharing a name are numbered name#1, name#2, ..."""
    F = r["conflict"]["newsFeeds"]
    n, k, out = Counter(f["name"] for f in F), Counter(), {}
    for f in F:
        k[f["name"]] += 1
        out[f"rss:{f['name']}" + (f"#{k[f['name']]}" if n[f["name"]] > 1 else "")] = f
    return out


def sources(r):
    for sid, f in feeds(r).items():
        yield sid, CADENCE, partial(fetch, f["url"], f["name"])


def fetch(url, name):
    return parse(get(url, headers={"Accept": "application/rss+xml, application/xml, */*"}).content,
                 name)


def _local(e):
    return e.tag.rsplit("}", 1)[-1]


def _child(e, name):
    return next((c for c in e if _local(c) == name), None)


def _text(e, name):
    c = _child(e, name)
    return (c.text or "").strip() if c is not None else ""


def date(s):
    """RFC 822 (RSS) or ISO 8601 (Atom) timestamp in UTC; None if unparseable."""
    if not s:
        return None
    try:
        t = parsedate_to_datetime(s)
    except (TypeError, ValueError):
        try:
            t = datetime.fromisoformat(s.replace("Z", "+00:00"))
        except ValueError:
            return None
    return t.astimezone(UTC) if t.tzinfo else t.replace(tzinfo=UTC)


def parse(xml, name):
    """Items of an RSS or Atom document, first PER_FEED, as collection items."""
    root = ET.fromstring(xml)
    E = [e for e in root.iter() if _local(e) == "item"] or \
        [e for e in root.iter() if _local(e) == "entry"]
    items = []
    for e in E[:PER_FEED]:
        title = _text(e, "title")
        if not title:
            continue
        if name == "Google News" and title.rfind(" - ") > 0:   # strip " - Outlet"
            title = title[:title.rfind(" - ")]
        link = _text(e, "link")
        if not link and (l := _child(e, "link")) is not None:
            link = l.get("href", "")
        pub = _text(e, "pubDate") or _text(e, "published") or _text(e, "updated")
        items.append({
            "native_id": link or title, "published_at": date(pub), "url": link,
            "payload": {"title": title[:EXCERPT], "link": link, "source": name, "pubDate": pub,
                        "category": _text(e, "category") or None},
        })
    return items
