"""Market signals, as upstream src/app/api/{markets,oil,crypto,polymarket}:
Yahoo Finance chart meta per symbol, CoinGecko spot prices, Polymarket markets
that match any region's keywords. A snapshot is stored when its value changes."""
import json
from functools import cache
from pathlib import Path

from engine import regions
from engine.collect import epoch
from engine.collect.rss import encode_uri_component

CADENCE = 600
YAHOO = "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d"
POLY = ("https://gamma-api.polymarket.com/markets?limit=500&closed=false&active=true"
        "&order=volume24hr&ascending=false")
BROWSER = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"}


@cache
def symbols():
    return json.loads((Path(regions.DIR) / "global.json").read_text(encoding="utf8"))


def sources(R):
    g = symbols()
    # markets route encodes the symbol; the oil route inserts it raw
    S = [{"id": f"markets:Yahoo Finance#{s}", "region": "global", "kind": "yahoo",
          "url": YAHOO.format(encode_uri_component(s)), "cadence": CADENCE, "headers": BROWSER}
         for s in g["markets"]]
    S += [{"id": f"markets:Yahoo Finance#{s}", "region": "global", "kind": "yahoo",
           "url": YAHOO.format(s), "cadence": CADENCE, "headers": BROWSER} for s in g["oil"]]
    S.append({"id": "markets:CoinGecko", "region": "global", "kind": "coingecko",
              "cadence": CADENCE, "headers": {"Accept": "application/json"},
              "url": ("https://api.coingecko.com/api/v3/simple/price?ids="
                      f"{','.join(g['coins'])}&vs_currencies=usd&include_24hr_change=true")})
    S.append({"id": "markets:Polymarket", "region": "global", "kind": "polymarket",
              "url": POLY, "cadence": CADENCE})
    return S


def parse(body, src):
    d = json.loads(body)
    if src["kind"] == "yahoo":
        m = d["chart"]["result"][0]["meta"]
        t = epoch(1000 * m["regularMarketTime"])
        return [{"native_id": f"{m['symbol']}@{m['regularMarketTime']}", "published_at": t,
                 "url": None, "payload": {k: m.get(k) for k in (
                     "symbol", "regularMarketPrice", "chartPreviousClose", "currency")}}]
    if src["kind"] == "coingecko":
        return [{"native_id": f"{c}@{v.get('usd')}", "published_at": None, "url": None,
                 "payload": {"coin": c, **v}} for c, v in d.items()]
    R = regions.regions().values()
    K = [(regions.regex(r["conflict"]["polymarketKeywords"]),
          regions.regex(r["conflict"]["polymarketExclude"]), r["id"]) for r in R]
    items = []
    for m in d:
        q = m.get("question") or ""
        rs = [rid for k, x, rid in K if k.search(q) and not x.search(q)]
        if rs:
            items.append({"native_id": f"{m['id']}@{m.get('outcomePrices')}",
                          "published_at": None, "url": f"https://polymarket.com/event/{m.get('slug')}",
                          "payload": {"id": m["id"], "question": q, "regions": rs,
                                      **{k: m.get(k) for k in ("outcomes", "outcomePrices",
                                                               "volume24hr", "endDate")}}})
    return items
