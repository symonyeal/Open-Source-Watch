"""Air-raid alerts, as upstream src/app/api/alerts: Tzeva Adom (Pikud HaOref
mirror) for alertProvider "tzevaadom", alerts.com.ua oblast states for "alertsua".
An item is one alert; a continuing alert repeats its native id and is stored once."""
import hashlib
import json

from engine.collect.rss import date

CADENCE = 15
URLS = {"tzevaadom": "https://api.tzevaadom.co.il/notifications",
        "alertsua": "https://alerts.com.ua/api/states"}


def sources(R):
    return [{"id": f"alerts:{p}", "region": rid, "url": URLS[p], "cadence": CADENCE,
             "provider": p, "headers": {"Accept": "application/json"}}
            for rid, r in R.items() if (p := r["conflict"]["alertProvider"]) in URLS]


def parse(body, src):
    d = json.loads(body)
    if src["provider"] == "alertsua":
        return [{"native_id": f"{s.get('id')}@{s.get('changed')}",
                 "published_at": date(s.get("changed")), "url": None,
                 "payload": {"provider": "alertsua", "threat": "Air Raid Alert",
                             "locations": [s.get("name_en") or s.get("name") or "Unknown"]}}
                for s in d.get("states", []) if s.get("alert")]
    items = []
    for a in d if isinstance(d, list) else []:
        threat = a.get("threat") or a.get("title") or "Alert"
        cities = a["cities"] if isinstance(a.get("cities"), list) else [a.get("data") or "Unknown"]
        key = json.dumps([a.get("date"), threat, cities], ensure_ascii=False)
        items.append({"native_id": hashlib.sha256(key.encode()).hexdigest()[:16],
                      "published_at": date(a.get("date")), "url": None,
                      "payload": {"provider": "tzevaadom", "threat": threat, "locations": cities}})
    return items
