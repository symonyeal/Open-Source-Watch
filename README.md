# Open-Source-Watch

**Dashboard:** https://symonyeal.github.io/Open-Source-Watch/ is a static snapshot of
the stored data, rebuilt after each twice-daily collection; the header gives its time.
Run it locally (below) for live feeds.

## In plain terms

Wars and crises leave a trail in public information: news reports, official Telegram
channels, air-raid sirens, aircraft transponders, satellite fire detections and market
prices. Open-Source-Watch gathers that trail for a region, keeps every item with a note
of where it came from and how far that source can be trusted, and shows it on one live
map.

Select **SOUTH ASIA** for Bangladesh, India, Pakistan, Sri Lanka, Myanmar, Nepal,
Bhutan and Maldives. The **ENERGY & TRADE** view adds dated oil benchmarks, port
activity and country energy-import context alongside the incident monitor.
See [Data context](docs/DATA-CONTEXT.md) for sources, calculations and limits.
South Asia incident indicators are reporting counts, with no calibrated alert
cutoffs. [Maritime methods](docs/MARITIME-METHODS.md) records the satellite
benchmarks and reference implementations assessed for further work.

**Status.** All four phases of the [design](docs/specs/2026-10-20-open-source-watch-design.md):
every feed of the two starting regions (Iran/Israel, Russia/Ukraine) is collected,
graded and stored by the Python engine, which serves it to the dashboard. That covers
news, Telegram, satellite fire detections, aircraft, air-raid alerts, drone tracks and
markets. The footer shows the health of each source.
Phase 2 adds fusion: reports of the same kind close in time and place are merged
into events, each graded by how many independent, reliable sources support it. The
header shows each region's priority intelligence requirements as indicators. A
scheduled GitHub job collects the slower feeds twice a day into the `data`
branch, so history builds while the laptop is off.
Phases 3 and 4 add four shipping chokepoints (Red Sea, Strait of Hormuz, Taiwan Strait,
Black Sea), each defined by one file, and written products: a daily intelligence
summary (INTSUM) and a six-hourly situation report (SITREP), with competing hypotheses
weighed against the evidence. Fire detections at permanent heat sources such as gas
flares no longer count as anomalies, and a trained classifier replaced keyword rules
after measuring better. Ship traffic through each strait is counted from AIS data
when a free aisstream.io key is set. Only observed gate crossings between positions at
most 15 minutes apart count. A day is sampled when a valid segment has an endpoint on
it; this does not establish full-day coverage, and the policy has not been validated
against live AIS. Static naval positions are withheld until dated positions are available.

**What this does not show.** Nothing here is verified ground truth. Each item is a
claim by its source; the grade says how reliable the source has been judged, not whether
the claim is true. Gaps in collection are recorded, and are not quiet periods.

## Overview

A fork of [IRONSIGHT](https://github.com/NoblerWorks-HQ/IRONSIGHT) by
[Nobler Works](https://noblerworks.com/) (MIT). IRONSIGHT's Next.js dashboard is kept as
the common operational picture. A Python engine (`engine/`) adds the stages of the
intelligence cycle that a live viewer lacks:

| stage | module | output |
| --- | --- | --- |
| direction | `regions/<id>/region.toml` | PIRs, indicators, source grades |
| collection | `engine/collect` | immutable raw reports with provenance |
| processing | `engine/process` | observations: English text, place, time, kind |
| fusion, analysis, production | `engine/fuse`, `analyze`, `produce` | events, indicator status, summaries (phases 2–3) |
| dissemination | IRONSIGHT front end, engine API | map, panels, alerts |

Sources carry Admiralty reliability grades (A–F); observations carry credibility (1–6),
computed from corroboration. Events use ACLED's taxonomy. Coverage grows chokepoint by
chokepoint: the Red Sea, Hormuz, the Taiwan Strait and the Black Sea come next.

### Run

```powershell
py -3.14 -m pip install -r requirements.txt
py -3.14 -m pip install ./kernels                 # Rust fusion kernel (needs cargo)
py -3.14 -m engine serve                          # engine on http://127.0.0.1:8790
```

In a second PowerShell terminal:

```powershell
npm.cmd ci
$env:ENGINE_URL = 'http://127.0.0.1:8790'
npm.cmd run dev                                  # dashboard on http://localhost:3000
```

Without `ENGINE_URL`, the incident feeds use their direct sources and the
Energy & Trade view reports that its engine is unavailable. Set
`AISSTREAM_KEY` (free at aisstream.io) before `engine serve` to count ship transits.
To add a region, copy `regions/red-sea/region.toml`, edit it, and run
`py -3.14 -m engine regions`. Data is written
to `./data` (or `$OSW_DATA`).

To collect only the new public context sources, run
`py -3.14 -m engine collect --adapter context`. No key is needed for FRED,
PortWatch or World Bank data. A local engine polls oil and shipping every six
hours and country indicators daily; the twice-daily collection workflow includes them. These are
publication cadences for collection, not promises about provider update times.

Tests: `py -3.14 -m pytest -q` and `npm test`. Fusion needs the kernel; collection does not.

---

## Upstream: IRONSIGHT

The dashboard is [IRONSIGHT](https://github.com/NoblerWorks-HQ/IRONSIGHT) by Nobler Works
(MIT); its feature list is in the upstream README. Its data sources and attribution follow.

## Data Sources

**All data comes from free, publicly accessible endpoints and requires no API keys.** None of it is owned by this project — see [Credits & Attribution](#credits--attribution).

### Live APIs & Feeds

| Service | Data | Provider | Cost |
|---------|------|----------|------|
| Yahoo Finance | Stocks, indices, commodities, oil futures | Yahoo | Free, no key |
| CoinGecko | Cryptocurrency prices (BTC, ETH, SOL, BNB) | CoinGecko | Free, no key |
| Polymarket | Prediction-market odds | Polymarket (Gamma API) | Free, no key |
| NASA FIRMS | Fire / thermal-anomaly satellite data | NASA | Free, no key |
| adsb.lol | Military aircraft ADS-B tracking | Community ADS-B network | Free, no key |
| Tzeva Adom | Israeli missile/rocket/drone alerts | Community mirror of Pikud HaOref | Free, no key |
| alerts.com.ua | Ukrainian oblast air-raid alerts (EN names) | alerts.com.ua | Free, no key |
| Neptun | Ukraine real-time drone/missile/KAB tracks | neptun.in.ua | Free, no key |
| Google News RSS | Keyword/location-scoped news aggregation | Google | Free, no key |
| Google Translate | Hebrew/Arabic/Farsi + Ukrainian/Russian translation | Google (unofficial) | Free, no key |
| Telegram | Public channel posts via embed scraping | Telegram | Free, no key |

### Map

| Layer | Provider | License |
|-------|----------|---------|
| Dark basemap tiles | [Esri](https://www.esri.com/) World Dark Gray Canvas (Esri, HERE, Garmin, © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors) | Esri attribution |
| Country border lines | [Natural Earth](https://www.naturalearthdata.com/) `ne_50m_admin_0_boundary_lines_land` | Public domain |
| Map engine | [Leaflet](https://leafletjs.com/) | BSD-2-Clause |

### News RSS Feeds

<details>
<summary><strong>Iran / Israel theater</strong></summary>

BBC Middle East, New York Times Middle East, Al Jazeera, Reuters World, CNN Middle East, Fox News World, Wall Street Journal, Times of Israel, Jerusalem Post, Ynet News, N12 (Mako), Walla News, Haaretz, PressTV (Iran), The National (UAE/GCC), Drop Site News, Breaking Defense, Long War Journal, Military Times, War on the Rocks, CENTCOM, U.S. DoD, and conflict-scoped Google News searches.
</details>

<details>
<summary><strong>Russia / Ukraine theater</strong></summary>

BBC Europe, New York Times Europe, Al Jazeera, Reuters World, Fox News World, Wall Street Journal, Kyiv Independent, Ukrainska Pravda (EN), Kyiv Post, The New Voice of Ukraine (NV), Ukrinform, TASS, RT, The Moscow Times, Meduza (EN), Breaking Defense, Military Times, War on the Rocks, U.S. DoD, and war-scoped Google News searches.
</details>

### Telegram Channels

Public channels only, scraped via Telegram's public embed endpoint. Channels are periodically activity-checked; the Russia/Ukraine set was last verified for recent activity in July 2026.

<details>
<summary><strong>Iran / Israel theater</strong></summary>

IDF Official, Rocket Alert, Alert Israel, Times of Israel, Abu Ali Express, OSINT Defender, Warfare Analysis, RN Intel, GeoPol Watch, ME Spectator (×2), Hamas-Israel War, PressTV, Iran International, Fars News, Tasnim News, Saberin (IRGC), DefaPress (Iran MOD), IRGC Official, Fotros Resistance, Quds News, Al-Saa EN, The Cradle, Drop Site News, France 24, WAM (UAE), Gulf News (UAE), Ali Bk, Al Jazeera, Bint Jbeil, Kian Meli (Iran).
</details>

<details>
<summary><strong>Russia / Ukraine theater</strong></summary>

**Ukrainian official / military:** UA General Staff, UA Air Force (kpszsu), Zelensky Official, Operatyvno ZSU, Ukraine NOW.

**Ukrainian outlets:** Kyiv Independent, Ukrainska Pravda, UNIAN, Suspilne, Trukha UA, NEXTA, Insider UA, LIGA-adjacent aggregators, BBC Ukraine, Radio Svoboda.

**Ukrainian journalists / independents:** Yuriy Butusov, Serhii Sternenko, Serhii Flash, Lachen, Anton Gerashchenko (EN), Tsaplienko, Motolko (Belarus).

**Russian state / war correspondents:** RIA Novosti, Russian MoD, Readovka, WarGonzo, RV Voenkor, Poddubny, Colonel Cassad, Grey Zone, Two Majors, Kots, Kotenok.

**Independent Russian journalists / outlets:** Meduza, Astra, Mediazona, Agentstvo, Holod, Maxim Katz.

**OSINT aggregators:** OSINT Defender, War Monitor, DeepState UA, Rybar.
</details>

### Polling Intervals

| Feed | Interval |
|------|----------|
| Air-raid alerts (Pikud HaOref / alerts.com.ua) | 15 seconds |
| Drone / missile tracker (Neptun) | 20 seconds |
| Telegram channels | 60 seconds |
| Regional threat monitor | 60 seconds |
| News RSS | 90 seconds |
| Strikes | 2 minutes |
| Conflicts / Flights | 3 minutes |
| Naval | 5 minutes |
| Markets, Oil, Crypto & Polymarket | 10 minutes |
| Fires (NASA FIRMS) | 10 minutes |

## Credits & Attribution

IRONSIGHT is a **viewer**, not a data owner. Every feed, dataset, headline, alert, track, tile, and post surfaced by this dashboard is the property of its respective provider and is used here, unmodified in substance, purely to point back to the original source. We claim **no ownership or credit** for any third-party data or open-source project listed below, and we are grateful to the people and communities who make them freely available:

- **News:** the news organizations and wire services listed above, via their public RSS feeds
- **Air-raid alerts:** [Pikud HaOref](https://www.oref.org.il/) via the community Tzeva Adom mirror; Ukrainian oblast alerts via [alerts.com.ua](https://alerts.com.ua/)
- **Drone / missile tracking:** [Neptun](https://neptun.in.ua/) (neptun.in.ua)
- **Aircraft:** [adsb.lol](https://www.adsb.lol) community ADS-B network
- **Satellite thermal:** [NASA FIRMS](https://firms.modaps.eosdis.nasa.gov/)
- **Markets / energy:** Yahoo Finance; **crypto:** [CoinGecko](https://www.coingecko.com/); **prediction markets:** [Polymarket](https://polymarket.com/)
- **Maps:** [Leaflet](https://leafletjs.com/), basemap by [Esri](https://www.esri.com/) with data © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors, borders from [Natural Earth](https://www.naturalearthdata.com/)
- **Translation:** Google Translate (unofficial endpoint)
- **Social OSINT:** public Telegram channels, owned by their respective operators

If you are a provider listed here and would like your source removed or its attribution changed, please [contact us](https://noblerworks.com/).

## Legal Disclaimer

### Purpose and Scope

This project is provided strictly for **educational and research purposes**. It demonstrates techniques for aggregating publicly available open-source intelligence (OSINT) using modern web technologies. It is not intended for commercial use, resale of data, or any activity that violates applicable laws or third-party terms of service.

### Data Sources

All data is sourced from publicly accessible endpoints. No paywalls are bypassed, no authentication is circumvented, and no copyrighted content is reproduced in full — only headlines, links, and publicly available metadata are displayed.

### Unofficial Endpoints

Some data sources rely on **unofficial or undocumented public endpoints**, including but not limited to: Yahoo Finance chart data, Google Translate, Telegram channel embeds, Google News RSS, and community alert/tracking feeds. These endpoints:

- Are not officially supported APIs and may violate the respective provider's Terms of Service
- May stop working, change, or be blocked without notice
- Are used here solely for non-commercial educational demonstration
- Should be replaced with official APIs if you intend to use this project commercially or in production

### Third-Party Content

News content, Telegram posts, financial data, alert data, drone/missile tracks, map tiles, and all other third-party content belong to their respective publishers, organizations, and data providers. This project does not claim ownership of, or credit for, any third-party content. Market data may be subject to additional redistribution restrictions from upstream data licensors.

### User Responsibility

By using this software, you agree that:

- **You are solely responsible** for ensuring your use complies with all applicable laws and third-party terms of service in your jurisdiction
- The authors and contributors of this project are **not liable** for any misuse, TOS violations, legal claims, or damages arising from use of this software
- You will **not use this software** for commercial data redistribution, automated trading, or any purpose that violates the terms of the underlying data providers
- This software is provided **"as is"** without warranty of any kind

### ADS-B Data Attribution

Military aircraft tracking data is provided by [adsb.lol](https://www.adsb.lol) under the [Open Database License (ODbL 1.0)](https://opendatacommons.org/licenses/odbl/1-0/).

### Map Data Attribution

Basemap tiles © Esri, HERE, Garmin, map data © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors. Country boundary lines from [Natural Earth](https://www.naturalearthdata.com/) (public domain).

### No Endorsement

This project is not affiliated with, endorsed by, or sponsored by any of the data providers, news organizations, governments, or military entities whose data it aggregates.

## License

[MIT](LICENSE)
