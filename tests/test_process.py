import pytest

from engine import process, regions
from engine.process import news

R = regions.regions()
II, RU = R["iran-israel"], R["russia-ukraine"]


def test_relevance_rule_of_upstream():
    K = regions.regex(II["conflict"]["newsRelevanceKeywords"])
    assert not news.relevant("Iran fans celebrate World Cup win", None, True, K)  # noise first
    assert news.relevant("Local weather", None, True, K)                         # unfiltered
    assert not news.relevant("Local weather", None, False, K)
    assert news.relevant("Hezbollah statement", None, False, K)
    assert news.relevant("Statement", "Gaza", False, K)                          # category


@pytest.mark.parametrize("s, kind", [
    ("Sirens sound across central Israel", "air_raid_alert"),
    ("Houthis claim missile attack on tanker in Red Sea", "maritime_incident"),
    ("Drone strikes hit Kuwait airport", "explosions_remote_violence"),
    ("Heavy fighting near Pokrovsk", "battles"),
    ("Thousands protest in Tel Aviv", "protests"),
    ("Ceasefire talks resume in Doha", "strategic_developments"),
    ("Opinion: what comes next", "report"),
])
def test_classify(s, kind):
    assert process.classify(s) == kind


@pytest.mark.parametrize("rid, s, place", [
    ("iran-israel", "Fujairah Oil Zone Hit by Fire After Drone Attack", "Fujairah"),
    ("russia-ukraine", "In the Poltava community, UAVs were hit", "Poltava"),
])
def test_geonames_city_and_short_words(rid, s, place):
    """GeoNames names match capitalised; 'Hit' (Iraq, under 500,000) is not a place."""
    names = process.geocode(process.gazetteer(R[rid]), s)[3]
    assert place in names and "Hit" not in names


def test_geocode_prefers_the_most_specific_place():
    G = process.gazetteer(II)
    lat, lon, prec, names = process.geocode(G, "Strike near Isfahan, Iran")
    assert (lat, lon, prec) == (32.65, 51.67, "city") and set(names) == {"isfahan", "iran"}
    assert process.geocode(G, "Nothing here")[2] == "none"


def test_single_observation_credibility():
    assert process.credibility_single(II, "rss:BBC") == 3       # B
    assert process.credibility_single(II, "rss:PressTV#2") == 4  # D, numbered feed
    assert process.credibility_single(II, "rss:Unknown") == 6    # ungraded = F


def test_language_by_script():
    assert [process.lang(s) for s in ["hello", "שלום", "مرحبا", "привіт"]] == \
        ["en", "he", "ar", "ru"]


def test_translation_is_cached(monkeypatch):
    process._cache["שלום"] = "Hello"
    monkeypatch.setattr(process, "google", None)  # any network call would fail
    assert process.translate("שלום") == "Hello" and process.translate("hi") == "hi"
