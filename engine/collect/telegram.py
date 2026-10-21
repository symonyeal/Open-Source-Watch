"""Public Telegram channels via the web preview t.me/s/<channel>, which lists the
latest posts in one request. Upstream (src/app/api/telegram) probes post ids one
embed page at a time; the preview needs one request per channel per poll.
Channels with the preview disabled return no posts: status `empty`."""
import html
import re

from engine.collect import EXCERPT
from engine.collect.rss import date

CADENCE = 120
POST = re.compile(r'data-post="([^"/]+)/(\d+)"')
TEXT = re.compile(r'<div class="tgme_widget_message_text js-message_text"[^>]*>(.*?)</div>',
                  re.DOTALL)
TIME = re.compile(r'<time[^>]*datetime="([^"]+)"')


def sources(R):
    return [{"id": f"telegram:{c['name']}", "region": rid, "url": f"https://t.me/s/{c['name']}",
             "cadence": CADENCE, "channel": c["name"]}
            for rid, r in R.items() for c in r["conflict"]["telegramChannels"]]


def text(fragment):
    """Plain text of a post body, as upstream src/lib/telegramText.ts."""
    s = re.sub(r"<br\s*/?>", " ", fragment, flags=re.IGNORECASE)
    s = html.unescape(re.sub(r"<[^>]+>", "", s))
    return re.sub(r"\s+", " ", s).strip()


def parse(body, src):
    page = body.decode("utf8", "replace")
    if "tgme_" not in page:
        raise ValueError("not a Telegram page")
    parts = POST.split(page)[1:]          # [channel, id, html, channel, id, html, ...]
    items = []
    for k in range(0, len(parts), 3):
        ch, pid, block = parts[k], int(parts[k + 1]), parts[k + 2]
        m, t = TEXT.search(block), TIME.search(block)
        if not m or not (s := text(m.group(1))):
            continue
        url = f"https://t.me/{ch}/{pid}"
        items.append({"native_id": pid, "published_at": date(t.group(1)) if t else None,
                      "url": url, "payload": {"channel": ch, "postId": pid,
                                              "text": s[:EXCERPT], "date": t and t.group(1)}})
    return items
