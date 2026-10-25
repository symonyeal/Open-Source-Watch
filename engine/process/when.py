"""Event time stated in a report (spec P3, needed by contradiction).

A report's timestamp is its publication; what it describes may be hours earlier.
Recognised, in English text, local to the region's time zone:
    a clock time   "5:20 a.m.", "11 pm", "05:20 local time", "at 0520 local"
    an optional date after it   "on Oct. 21", "on October 21"
Without a date, the event day is the publication day, or the day before if the
clock time would lie after publication. A dated time after publication is an
announcement, not a report, and yields None. Anything vaguer ("overnight", "this
morning"), or impossible ("0 pm", "Feb. 30"), yields None: no time is better than
a guessed one.
"""
import re
from datetime import datetime, timedelta
from zoneinfo import ZoneInfo

MONTHS = {m: k for k, m in enumerate(
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"], 1)}
CLOCK = re.compile(
    r"\b(?P<h>\d{1,2})(?::(?P<m>\d{2}))?\s*(?P<ap>[ap])\.?\s?m\b\.?"
    r"|\b(?P<H>[01]?\d|2[0-3]):?(?P<M>[0-5]\d)\s*(?:\(?local(?: time)?\)?)", re.IGNORECASE)
DATE = re.compile(r"^\W*(?:local time\W*)?on\s+(?P<mon>[A-Za-z]{3})[a-z]*\.?\s+(?P<d>\d{1,2})\b",
                  re.IGNORECASE)


def event_time(text, published, tz):
    """UTC datetime of the first clock time stated in text, or None."""
    m = CLOCK.search(text or "")
    if not m:
        return None
    if m["h"]:
        if not 1 <= int(m["h"]) <= 12 or int(m["m"] or 0) > 59:
            return None
        h, mi = int(m["h"]) % 12 + (12 if m["ap"].lower() == "p" else 0), int(m["m"] or 0)
    else:
        h, mi = int(m["H"]), int(m["M"])
    z = ZoneInfo(tz)
    pub = published.astimezone(z)
    d = DATE.match(text[m.end():])
    if d and d["mon"].lower() in MONTHS:
        try:
            t = datetime(pub.year, MONTHS[d["mon"].lower()], int(d["d"]), h, mi, tzinfo=z)
            if t - pub > timedelta(days=180):             # "Dec. 31" read on Jan. 2
                t = t.replace(year=pub.year - 1)
        except ValueError:                                # "Feb. 30"
            return None
        if t > pub:                                       # announced, not yet happened
            return None
    else:
        t = pub.replace(hour=h, minute=mi, second=0, microsecond=0)
        if t > pub:
            t -= timedelta(days=1)
    return t.astimezone(ZoneInfo("UTC"))
