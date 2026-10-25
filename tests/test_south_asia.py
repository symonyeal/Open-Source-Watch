import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGION = tomllib.loads((ROOT / "regions/south-asia/region.toml").read_text(encoding="utf8"))
COUNTRIES = {
    "BGD": "Bangladesh",
    "IND": "India",
    "PAK": "Pakistan",
    "LKA": "Sri Lanka",
    "MMR": "Myanmar",
    "NPL": "Nepal",
    "BTN": "Bhutan",
    "MDV": "Maldives",
}


def test_scope_and_map_cover_all_eight_countries():
    assert set(REGION["context"]["countries"]) == set(COUNTRIES)
    cities = REGION["conflict"]["client"]["cities"]
    assert {city["country"] for city in cities} == set(COUNTRIES.values())
    assert set(REGION["conflict"]["client"]["cityColors"]) >= set(COUNTRIES.values())
    assert {clock["zone"] for clock in REGION["conflict"]["client"]["timeZones"]} >= {
        "Asia/Dhaka", "Asia/Kolkata", "Asia/Karachi", "Asia/Colombo",
        "Asia/Yangon", "Asia/Kathmandu", "Asia/Thimphu", "Indian/Maldives", "UTC",
    }


def test_each_country_has_a_news_query_for_relevant_context():
    queries = REGION["conflict"]["server"]["countryQueries"]
    assert {query["country"] for query in queries} == set(COUNTRIES.values())
    for query in queries:
        text = query["query"].lower()
        assert any(term in text for term in ("port", "shipping", "fuel", "power", "hydro"))
        assert any(term in text for term in (
            "flood", "cyclone", "earthquake", "security", "conflict", "protest"))
        assert "cricket" in text or "football" in text


def test_south_asia_google_queries_are_grouped_with_explicit_geography():
    server = REGION["conflict"]["server"]
    assert server["filterGoogleNews"] is True
    country_names = {query["country"] for query in server["countryQueries"]}
    assert country_names == {
        "Bangladesh", "India", "Pakistan", "Sri Lanka", "Myanmar", "Nepal", "Bhutan", "Maldives",
    }
    for query in server["strikeQueries"] + server["conflictQueries"]:
        assert query.startswith("(") and query.endswith(")")
        assert query.count("(") >= 2
        assert all(country.split()[0] in query for country in country_names)
    assert all("+" in query and "%22" in query for query in server["strikeQueries"])
    assert all("+" not in query and "%22" not in query for query in server["conflictQueries"])
    assert all('"Sri Lanka"' in query for query in server["conflictQueries"])


def test_google_news_filter_drops_live_off_region_and_entertainment_titles():
    from engine import regions
    from engine.process import news

    region = regions.regions()["south-asia"]
    conflict = region["conflict"]
    keywords = regions.regex(conflict["newsRelevanceKeywords"])
    countries = conflict["countryQueries"]
    rejected = [
        (
            "gnews:strike#2",
            (
                "Thailand imposes martial law in southern province after attack kills 5 "
                "paramilitary troops"
            ),
        ),
        ("gnews:conflict#1",
         "Bangkok flooding disrupts transport and container movements across the capital"),
        ("gnews:conflict#2",
         "Paris extends nighttime curfew for third day amid protests and unrest"),
        ("gnews:country#India", "Typhoon Champi batters Taiwan"),
        ("gnews:country#India",
         "Kartik Aaryan's 'Captain India' Set Amid Houthi Conflict in Yemen (Exclusive)"),
        ("gnews:country#India",
         "Bangladesh Bank website remains offline for third consecutive day"),
    ]
    assert all(not news.regional_google_news(title, source, keywords, countries)
               for source, title in rejected)
    assert news.regional_google_news(
        "India Orders Rooftop Solar Disconnection Following Nationwide Blackout",
        "gnews:country#India", keywords, countries)
    assert news.regional_google_news(
        "Burmese forces report clashes near a regional port",
        "gnews:country#Myanmar", keywords, countries)


def test_proven_and_maritime_feeds_are_graded():
    feeds = REGION["conflict"]["server"]["newsFeeds"]
    assert {feed["name"] for feed in feeds} == {"BBC Asia", "Al Jazeera", "gCaptain", "Splash247"}
    reliability = REGION["reliability"]
    assert all(reliability[f"rss:{feed['name']}"] in "ABCDEF" for feed in feeds)
    assert {"gnews:strike", "gnews:conflict", "gnews:country"} <= set(reliability)
    assert {
        "firms:VIIRS", "adsb:adsb.lol", "ais:aisstream", "markets:Yahoo Finance",
        "markets:CoinGecko", "markets:Polymarket",
    } <= set(reliability)


def test_pirs_use_event_taxonomy_without_economic_thresholds():
    pirs = REGION["pir"]
    assert {pir["id"] for pir in pirs} == {"SA-1", "SA-2", "SA-3"}
    kinds = {indicator["kind"] for pir in pirs for indicator in pir["indicator"]}
    assert {"maritime_incident", "battles", "violence_against_civilians", "protests"} <= kinds
    assert all(indicator.get("metric") == "event_count"
               and "watch" not in indicator and "trigger" not in indicator
               for pir in pirs for indicator in pir["indicator"])


def test_region_registry_can_build_the_conflict_config():
    from engine import regions

    region = regions.regions()["south-asia"]
    assert region["config"]["label"] == "SOUTH ASIA"
    assert region["context"]["countries"] == list(COUNTRIES)
    regions.regex(region["conflict"]["newsRelevanceKeywords"])
