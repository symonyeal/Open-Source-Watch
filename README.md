# Open-Source-Watch

Open-source intelligence dashboard for seven conflict and shipping regions. A Python
engine with Rust kernels stores public reports, grades sources, merges related reports
and produces regional indicators and summaries. The front end is based on
[IRONSIGHT](https://github.com/NoblerWorks-HQ/IRONSIGHT) by Nobler Works.

[Dashboard](https://symonyeal.github.io/Open-Source-Watch/). The published site is a
snapshot, rebuilt after twice-daily collection; its header gives the snapshot time.
Local operation supports live feeds.

## Coverage

The regions are Iran/Israel, Russia/Ukraine, Red Sea, Strait of Hormuz, Taiwan Strait,
Black Sea and South Asia. Configuration on 2026-10-29 listed 351 sources and 24 priority
intelligence requirements. Energy, shipping, navigation and connectivity measurements
provide context alongside news and incident reports.

## Processing

### Storage

Each news item, Telegram post, air-raid alert, military aircraft position and satellite fire
detection is stored as an immutable raw report with its source and fetch time.
The 2026-10-25 audit used 49,782 raw reports, 42,502 observations and 6,898 merged events.
Processing translates each report, finds the place and time it describes, and
classifies what kind of event it reports, producing an *observation*. Because raw
reports are never changed, everything downstream can be rebuilt from them, and a test
checks that a rebuild reproduces the same tables.

The engine serves IRONSIGHT's 18 data routes through a recording proxy: when the
dashboard asks a news site or tracking service for data, the engine returns its stored
response. In the
2026-10-21 route test, the engine handled all 106 requests: 103 returned HTTP 200 and
three returned HTTP 502 from failed upstream feeds.

### Source reliability and report credibility

Every source carries an Admiralty reliability grade (A, completely reliable, to F,
cannot be judged). A report's credibility (1, confirmed, to 6, cannot be judged) is computed
from how many *independent* reliable sources say the same thing. Channels with the same
owner count once; Google News items are credited to the outlet that wrote
them, not to Google. A report contradicted by a silent warning system scores 5,
improbable.

### Event fusion

Reports of the same kind of event within two hours and 25 km of each other are merged
into one event, using ACLED's conflict taxonomy. Country-level locations do not link
otherwise unrelated incidents. In the 2026-10-25 audit, Rust linkage took 0.06 seconds
against 0.54 for the NumPy implementation, and 0.58 against 4.8 at nine times the history.
Tests compare the result against a brute-force check of every pair.

### Indicators and reports

Each region's configuration lists sources, grades and questions, such as whether a direct
missile or drone exchange between Iran and Israel is under way. Each question has
indicators with time windows and thresholds, shown in the header strip. A source that
fails is reported as a gap: a failed
feed makes its indicator *no data*, not *inactive*. Competing explanations are ranked by
how much evidence contradicts each one (analysis of competing hypotheses), and each
region gets a written daily summary (INTSUM) and six-hourly situation report (SITREP).

## Validation

- The starting keyword rules were scored on 200 labelled
  observations: 54 % correct. A trained text classifier reached 68 % under
  cross-validation and replaced them; rare event types remain weak for both.
- 1,206 of 1,435 satellite fire detections in one day over
  Iran/Israel were gas flares and other permanent heat sources. A detection now counts
  only if its 0.1° cell was active on fewer than 3 of the 7 previous days, which cut one strike
  indicator from 228 events to 42.
- A rule meant to flag reports contradicted by silent sirens fired
  falsely on live data: the reports were published hours after the alert they
  described. The rule now uses the event time stated in the text, and that case is a
  permanent test.
- Ship traffic through each strait is counted only from observed gate crossings
  between positions at most 15 minutes apart, and undated ship positions inherited from
  IRONSIGHT were removed instead of being shown as current.

The dated evidence and checks are recorded in [Validation](docs/VALIDATION.md), with
method choices in [Decisions](docs/DECISIONS.md).

## Energy and trade

Select SOUTH ASIA for Bangladesh, India, Pakistan, Sri Lanka, Myanmar, Nepal, Bhutan
and Maldives. The ENERGY & TRADE view adds dated oil spot prices (US Energy
Information Administration, via FRED), vessel traffic at four chokepoints and nine South
Asian ports (IMF PortWatch), and each country's energy and fuel imports (World Bank):
17 series, each with its date, unit and collection status. South Asia indicators are
reporting counts with no alert thresholds, because none have been validated. Satellite
ship-detection research was assessed but is not used as observation
([Maritime methods](docs/MARITIME-METHODS.md)).

## Navigation and connectivity

The SIGNALS view and GNSS map layer display four signal families for each
region ([method and limits](docs/SIGNALS.md)):

- Aircraft report how far their GPS position can be trusted.
  gpsjam.org counts, for each hexagonal cell and day, the aircraft that reported low
  accuracy; cells where more than 2 % or 10 % did are drawn on the map, by gpsjam.org's
  own formula. On 2026-10-22 to 28 Iran/Israel had 95 to 141 such cells over 10 % a
  day and the Taiwan Strait none. Jamming is one cause, not the only one, so the
  dashboard says "low accuracy", not "jamming". A local engine also computes the cells
  from the aircraft it already tracks; the cell arithmetic is a Rust kernel that
  reproduces all 47,482 of gpsjam.org's cell indices for one day.
- IODA (Georgia Tech) detects country-wide connectivity drops in
  routing, probing and telescope data; each outage becomes an event in the indicator
  board.
- Taiwan's defence ministry reports PLA aircraft sorties, ships, ADIZ entries and
  median-line crossings. The view shows about 90 days, beside each
  count's median over the previous 30 reports. It is a party's claim and is shown as one.
- UCDP, the Uppsala conflict dataset, codes each
  lethal incident with its date and location precision. `engine validate` measures
  how many of them the engine found. Not measured yet: UCDP's newest monthly release
  ends before this project's data begins.

## Published snapshot

A scheduled GitHub job collects the slower feeds twice a day into the `data` branch.
A second job then serves the stored data through the engine and dashboard once, saves
every view of every region (140 files), and publishes them as the static
dashboard. Files are grouped by month, so each row is rewritten at most twice.
The storage-layout benchmark in [Validation](docs/VALIDATION.md) recorded dashboard
requests taking 2.4 to 4 times less time.

## Limits

Each item is a source's claim. A reliability grade does not establish its truth.
Collection gaps are recorded separately from periods with no reported activity.

## Local use

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

- Without `ENGINE_URL`, the incident feeds use their direct sources and the Energy &
  Trade view reports that its engine is unavailable.
- Set `AISSTREAM_KEY` (free at aisstream.io) before `engine serve` to count ship transits.
- To add a region, copy `regions/red-sea/region.toml`, edit it, and run
  `py -3.14 -m engine regions`.
- Data is written to `./data` (or `$OSW_DATA`). `py -3.14 -m engine collect --adapter
  context` collects only the oil, shipping and country series; no key is needed. A local
  engine polls oil and shipping every six hours and country indicators daily.
- Tests: `py -3.14 -m pytest -q` and `npm test`. Fusion needs the kernel;
  collection does not.

## Project map

| Path | Contents |
| --- | --- |
| `engine/collect`, `engine/process` | collectors for each feed type and readers of what they store; translation, place, time and event type |
| `engine/fuse.py`, `kernels/` | merging reports into events; the Rust linking kernel |
| `engine/analyze.py`, `engine/produce.py` | indicators, competing hypotheses, INTSUM and SITREP |
| `engine/store.py`, `engine/api.py` | the Parquet store read with DuckDB; the engine's web API |
| `regions/<id>/region.toml` | one file per region: sources, grades, questions, indicators |
| `src/` | IRONSIGHT's Next.js dashboard, with the engine seams |
| `.github/workflows/` | tests, twice-daily collection, the static dashboard |
| [Design](docs/specs/2026-10-20-open-source-watch-design.md), [Decisions](docs/DECISIONS.md), [Validation](docs/VALIDATION.md) | the plan, each choice with its evidence, each check |
| [Data context](docs/DATA-CONTEXT.md), [Sources](docs/SOURCES.md), [Review log](docs/REVIEW-LOG.md) | energy and trade sources and limits; source research |

## Upstream: IRONSIGHT

The dashboard is [IRONSIGHT](https://github.com/NoblerWorks-HQ/IRONSIGHT) by Nobler Works
(MIT); its feature list is in the upstream README. Its data sources and attribution
follow; the region files list every source this project adds, with its grade.

## Data sources

The upstream feeds below use publicly accessible endpoints without API keys. The optional
AISSTREAM collection described above requires a key. Data belongs to its providers;
see [Credits & Attribution](#credits--attribution).

### APIs and feeds

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

### News RSS feeds

<details>
<summary><strong>Iran / Israel theater</strong></summary>

BBC Middle East, New York Times Middle East, Al Jazeera, Reuters World, CNN Middle East, Fox News World, Wall Street Journal, Times of Israel, Jerusalem Post, Ynet News, N12 (Mako), Walla News, Haaretz, PressTV (Iran), The National (UAE/GCC), Drop Site News, Breaking Defense, Long War Journal, Military Times, War on the Rocks, CENTCOM, U.S. DoD, and conflict-scoped Google News searches.
</details>

<details>
<summary><strong>Russia / Ukraine theater</strong></summary>

BBC Europe, New York Times Europe, Al Jazeera, Reuters World, Fox News World, Wall Street Journal, Kyiv Independent, Ukrainska Pravda (EN), Kyiv Post, The New Voice of Ukraine (NV), Ukrinform, TASS, RT, The Moscow Times, Meduza (EN), Breaking Defense, Military Times, War on the Rocks, U.S. DoD, and war-scoped Google News searches.
</details>

### Telegram channels

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

### Polling intervals

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

## Credits & attribution

The feeds, datasets, headlines, alerts, tracks, tiles and posts belong to their providers.
This project claims no ownership of third-party data or software. Provider attribution follows:

- **News:** the news organizations and wire services listed above, via their public RSS feeds
- **Air-raid alerts:** [Pikud HaOref](https://www.oref.org.il/) via the community Tzeva Adom mirror; Ukrainian oblast alerts via [alerts.com.ua](https://alerts.com.ua/)
- **Drone / missile tracking:** [Neptun](https://neptun.in.ua/) (neptun.in.ua)
- **Aircraft:** [adsb.lol](https://www.adsb.lol) community ADS-B network
- **Satellite thermal:** [NASA FIRMS](https://firms.modaps.eosdis.nasa.gov/)
- **Markets / energy:** Yahoo Finance; **crypto:** [CoinGecko](https://www.coingecko.com/); **prediction markets:** [Polymarket](https://polymarket.com/)
- **Maps:** [Leaflet](https://leafletjs.com/), basemap by [Esri](https://www.esri.com/) with data © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors, borders from [Natural Earth](https://www.naturalearthdata.com/)
- **Translation:** Google Translate (unofficial endpoint)
- **Social OSINT:** public Telegram channels, owned by their respective operators

For upstream provider removal or attribution requests, [contact Nobler Works](https://noblerworks.com/).

## Terms and limitations

### Use

This project is provided for education and research. It is not intended for commercial
use, data resale or activity that violates applicable laws or third-party terms.

### Access and content

Data comes from publicly accessible endpoints. The project does not bypass paywalls or
authentication or reproduce copyrighted articles in full. It displays headlines, links
and public metadata; stored article and post text is limited to 280 characters.

### Unofficial endpoints

Yahoo Finance chart data, Google Translate, Telegram embeds, Google News RSS and some
community alert and tracking feeds use unofficial or undocumented endpoints. These endpoints:

- Are not officially supported APIs and may violate the respective provider's Terms of Service
- May stop working, change, or be blocked without notice
- Are used here solely for non-commercial educational demonstration
- Should be replaced with official APIs if you intend to use this project commercially or in production

### Content rights

Market data may carry additional redistribution restrictions from upstream licensors.
Provider ownership and attribution are listed [above](#credits--attribution).

### User responsibility

By using this software, you agree that:

- You are solely responsible for compliance with applicable laws and third-party terms in your jurisdiction.
- The authors and contributors are not liable for misuse, terms-of-service violations, legal claims or damages arising from use of the software.
- You will not use the software for commercial data redistribution, automated trading or purposes that violate providers' terms.
- The software is provided "as is", without warranty.

### ADS-B attribution

Military aircraft tracking data is provided by [adsb.lol](https://www.adsb.lol) under the [Open Database License (ODbL 1.0)](https://opendatacommons.org/licenses/odbl/1-0/).

### Map attribution

Basemap tiles © Esri, HERE, Garmin, map data © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors. Country boundary lines from [Natural Earth](https://www.naturalearthdata.com/) (public domain).

### Affiliation

This project is not affiliated with, endorsed by, or sponsored by any of the data providers, news organizations, governments, or military entities whose data it aggregates.

## License

[MIT](LICENSE)
