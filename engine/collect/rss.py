"""RSS 2.0 and Atom feeds: the conflict's newsFeeds (upstream src/app/api/news) and
its Google News queries (src/app/api/{strikes,conflicts,regional-alerts}).

URLs are built exactly as upstream builds them, so the engine's stored response
answers the upstream route's request (spec § Seam): strikes and regional-alerts
insert the query raw, conflicts applies encodeURIComponent.
"""
from collections import Counter
from datetime import UTC, datetime
from email.utils import parsedate_to_datetime
from urllib.parse import quote
from xml.etree import ElementTree as ET

from engine.collect import EXCERPT

CADENCE = 300
PER_FEED = 25  # upstream reads the first 15 (news, strikes) or 25 (conflicts) items
GNEWS = "https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en"
ACCEPT = {"Accept": "application/rss+xml, application/xml, text/xml, */*"}


def feeds(r):
    """{source_id: feed}; feeds sharing a name are numbered name#1, name#2, ..."""
    F = r["conflict"]["newsFeeds"]
    n, k, out = Counter(f["name"] for f in F), Counter(), {}
    for f in F:
        k[f["name"]] += 1
        out[f"rss:{f['name']}" + (f"#{k[f['name']]}" if n[f["name"]] > 1 else "")] = f
    return out


def encode_uri_component(s):
    """ECMAScript encodeURIComponent (ECMA-262 §19.2.6.4)."""
    return quote(s, safe="-_.!~*'()")


def sources(R):
    S = []
    for rid, r in R.items():
        c = r["conflict"]
        S += [{"id": sid, "region": rid, "url": f["url"], "cadence": CADENCE, "name": f["name"],
               "headers": ACCEPT} for sid, f in feeds(r).items()]
        S += [{"id": f"gnews:strike#{k}", "region": rid, "url": GNEWS.format(q),
               "cadence": CADENCE, "name": "Google News"}
              for k, q in enumerate(c["strikeQueries"], 1)]
        S += [{"id": f"gnews:conflict#{k}", "region": rid,
               "url": GNEWS.format(encode_uri_component(q)), "cadence": CADENCE,
               "name": "Google News"} for k, q in enumerate(c["conflictQueries"], 1)]
        S += [{"id": f"gnews:country#{q['country']}", "region": rid,
               "url": GNEWS.format(q["query"]), "cadence": CADENCE, "name": "Google News"}
              for q in c["countryQueries"]]
    return S


def _local(e):
    return e.tag.rsplit("}", 1)[-1]


def _child(e, name):
    return next((c for c in e if _local(c) == name), None)


def _text(e, name):
    c = _child(e, name)
    return (c.text or "").strip() if c is not None else ""


def date(s):
    """RFC 2822 (RSS) or ISO 8601 (Atom) timestamp in UTC; None if unparseable."""
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


def parse(xml, src):
    """Items of an RSS or Atom document, first PER_FEED, as collection items."""
    name = src["name"]
    root = ET.fromstring(xml)
    E = [e for e in root.iter() if _local(e) == "item"] or \
        [e for e in root.iter() if _local(e) == "entry"]
    items = []
    for e in E[:PER_FEED]:
        title = _text(e, "title")
        if not title:
            continue
        outlet = ""
        if name == "Google News" and title.rfind(" - ") > 0:   # "Title - Outlet"
            title, outlet = title[:title.rfind(" - ")], title[title.rfind(" - ") + 3:]
        link = _text(e, "link")
        if not link and (l := _child(e, "link")) is not None:
            link = l.get("href", "")
        pub = _text(e, "pubDate") or _text(e, "published") or _text(e, "updated")
        items.append({
            "native_id": link or title, "published_at": date(pub), "url": link,
            "payload": {"title": title[:EXCERPT], "link": link, "source": name, "pubDate": pub,
                        "category": _text(e, "category") or None, "outlet": outlet or None},
        })
    return items
