"""AIS vessel positions from aisstream.io (spec P4): a websocket stream, so not
polled like the other sources. Needs a free API key in $AISSTREAM_KEY; without
one the source is recorded as `blocked` every minute and the transit PIRs show a
collection gap.

Subscription: the bounding box of every region that has a gate, PositionReport
messages only. Message fields follow the aisstream.io documentation
(MetaData.MMSI, ShipName, time_utc; Message.PositionReport.Latitude, Longitude,
Sog, Cog, TrueHeading).
"""
import asyncio
import json
import os
import re
from datetime import UTC, datetime

URL = "wss://stream.aisstream.io/v0/stream"
CADENCE = 60
TIME = re.compile(r"(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d)(?:\.(\d+))?")


def key():
    return os.environ.get("AISSTREAM_KEY")


def sources(R):
    return [{"id": "ais:aisstream", "region": "global", "url": URL, "cadence": CADENCE,
             "stream": True}]


def subscription(k, R):
    return {"APIKey": k, "FilterMessageTypes": ["PositionReport"],
            "BoundingBoxes": [[[r["bbox"][0], r["bbox"][2]], [r["bbox"][1], r["bbox"][3]]]
                              for r in R.values() if r["gate"]]}


def time_utc(s):
    """aisstream time_utc, e.g. '2026-10-21 08:00:01.123456789 +0000 UTC'."""
    m = TIME.match(s)
    t = datetime.strptime(m[1], "%Y-%m-%d %H:%M:%S").replace(tzinfo=UTC)
    return t.replace(microsecond=int((m[2] or "0")[:6].ljust(6, "0")))


def item(m):
    """Collection item of one aisstream message, or None if not a position."""
    if m.get("MessageType") != "PositionReport":
        return None
    md, p = m["MetaData"], m["Message"]["PositionReport"]
    t = time_utc(md["time_utc"])
    return {"native_id": f"{md['MMSI']}@{t.isoformat()}", "published_at": t, "url": None,
            "payload": {"mmsi": md["MMSI"], "name": (md.get("ShipName") or "").strip(),
                        "lat": p["Latitude"], "lon": p["Longitude"], "sog": p.get("Sog"),
                        "cog": p.get("Cog"), "heading": p.get("TrueHeading")}}


def parse(body, src):
    """Items of a batch of messages (one JSON message per line)."""
    return [x for line in body.decode("utf8").splitlines() if line.strip()
            if (x := item(json.loads(line)))]


async def _stream(k, R, put, stop):
    import websockets
    while not stop.is_set():
        try:
            async with websockets.connect(URL) as ws:
                await ws.send(json.dumps(subscription(k, R)))
                async for msg in ws:
                    put(msg if isinstance(msg, str) else msg.decode("utf8"))
                    if stop.is_set():
                        return
        except (OSError, websockets.WebSocketException):
            await asyncio.sleep(30)


def stream(k, R, put, stop):
    """Run the subscription until stop is set, calling put(message) per message;
    reconnect after 30 s on any connection failure."""
    asyncio.run(_stream(k, R, put, stop))
