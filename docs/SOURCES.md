# Sources

Methods, standards and endpoints the engine implements. Cite only what this file
records. Status is **inspected** (read for this project) or **cited** (not yet read).

| Source | Used for | Status |
| --- | --- | --- |
| [World Bank chokepoints notebook](https://worldbank.github.io/alternative-data-for-crisis/notebooks/disruptions-business-trade/chokepoints-monitor.html), data smoothing section | centered seven-day PortWatch mean; full-window restriction here | inspected 2026-10-24 |
| [Ship dataset catalog](https://github.com/jasonmanesis/Satellite-Imagery-Datasets-Containing-Ships), [HRSID](https://github.com/chaozhong2010/HRSID), [xView3 reference](https://github.com/DIUx-xView/xview3-reference), [iMoonLab](https://github.com/orgs/iMoonLab/repositories) | satellite method assessment; [scope and limits](MARITIME-METHODS.md) | inspected 2026-10-24; no detector installed |
| [EIA Brent / FRED](https://fred.stlouisfed.org/series/DCOILBRENTEU), [WTI](https://fred.stlouisfed.org/series/DCOILWTICO), graph CSV exports | Daily spot prices, USD/barrel; separate from futures | inspected and live responses 2026-10-24 |
| [IMF PortWatch](https://portwatch.imf.org/), public ArcGIS daily ports/chokepoints and reference layers | Daily total/tanker calls and transits; derived rolling averages | inspected and live responses 2026-10-24; see [context terms](DATA-CONTEXT.md) |
| World Bank WDI [EG.IMP.CONS.ZS](https://data.worldbank.org/indicator/EG.IMP.CONS.ZS), [TM.VAL.FUEL.ZS.UN](https://data.worldbank.org/indicator/TM.VAL.FUEL.ZS.UN), [API v2](https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures) | Country energy exposure, explicit ISO3 scope; CC BY 4.0 on indicator pages | inspected and live responses 2026-10-24 |
| Nobler Works, *IRONSIGHT*, github.com/NoblerWorks-HQ/IRONSIGHT, MIT, commit 854f335 | front end, feeds, relevance rule, endpoint list | inspected 2026-10-20 |
| NATO STANAG 2511 / AJP-2.1, source reliability A–F and information credibility 1–6 (the Admiralty code) | `reliability`, `credibility` | cited |
| ODNI, Intelligence Community Directive 203, *Analytic Standards*, 2015 | estimative language and confidence (P3) | cited |
| Joint Publication 2-0, *Joint Intelligence*, 2013, ch. I | the intelligence cycle and PIRs | cited |
| ACLED, *Armed Conflict Location & Event Data Codebook*, 2023 | event types (`KINDS`) | cited |
| RFC 4287, *The Atom Syndication Format*, 2005, §1.1 | Atom parsing test | inspected 2026-10-21 |
| RSS 2.0 specification, Harvard Berkman Center, 2003 | RSS parsing | cited |
| RFC 2822 §3.3 date-time | RSS `pubDate` parsing | cited |
| Google Translate unofficial endpoint `translate.googleapis.com/translate_a/single` | translation, as upstream | inspected 2026-10-21 (upstream `src/lib/hebrew.ts`) |
| GeoNames, `cities15000.zip`, download.geonames.org/export/dump, CC BY 4.0, readme.txt column list | gazetteer | inspected 2026-10-21 |
| NASA FIRMS, VIIRS S-NPP C2 active fire CSV; FIRMS FAQ on the confidence classes l/n/h | thermal anomalies | cited |
| adsb.lol API v2 (`/v2/mil`, `/v2/lat/{lat}/lon/{lon}/dist/{d}`), readsb `dbFlags` bits | aircraft tracks | inspected 2026-10-21 (live responses) |
| Tzeva Adom notifications API; alerts.com.ua `/api/states` | air-raid alerts | inspected 2026-10-21 (live responses; Tzeva Adom empty, test uses upstream's interface) |
| neptun.in.ua `/api/data` | drone and missile tracks | inspected 2026-10-21 (live response) |
| Yahoo Finance chart v8; CoinGecko `simple/price`; Polymarket Gamma `markets` | market signals | inspected 2026-10-21 (live responses) |
| Telegram public channel preview `t.me/s/<channel>` | Telegram posts | inspected 2026-10-21 (73 channels probed) |
| ECMA-262, §19.2.6.4 `encodeURIComponent` | URL parity with upstream | cited; checked against Node in tests |
| Heuer, R. J., *Psychology of Intelligence Analysis*, CIA Center for the Study of Intelligence, 1999, ch. 8 | ACH ranking by inconsistent evidence | cited |
| Elvidge, C. D. et al., "Methods for global survey of natural gas flaring from VIIRS data", *Energies* 9(1), 2016 | persistent heat sources (flares) in VIIRS detections | cited |
| U.S. EIA, *World Oil Transit Chokepoints*, 2024 | Hormuz carries about a fifth of world petroleum liquids consumption (link note) | cited |
| aisstream.io documentation, websocket API, PositionReport message | AIS adapter | cited (format only; no key, no live message) |
| scikit-learn 1.9: `TfidfVectorizer`, `LogisticRegression`, `StratifiedKFold` | kind classifier | inspected 2026-10-21 |
| Unicode 15.0 code charts: Hebrew U+0590, Arabic U+0600, Cyrillic U+0400 | language guess by script | cited |
| John Wiseman, [GPSJAM FAQ](https://gpsjam.org/faq): cell formula `100 (bad − 1) / (good + bad)`, bands 2 % and 10 %, data sources and incomplete days | `engine.gnss` cell rule and bands | inspected 2026-10-29 |
| gpsjam.org daily files `data/<day>-h3_4.csv` (hex, good, bad aircraft) and `data/manifest.csv` (date, suspect, source) | `engine.collect.gpsjam` | inspected 2026-10-29 (live responses; 2026-10-27: 47,482 cells) |
| Bilawal Sidhu, *God's Eye View*, github.com/bilawalsidhu/gods-eye-view, MIT, commit bcabdff, `src/layers/gnss/records.js` | per-aircraft rule and validity filters of `engine.collect.integrity` | inspected 2026-10-29 |
| 14 CFR 91.227(c), ADS-B Out equipment performance requirements (NACp ≥ 8, NIC ≥ 7) | source of the per-aircraft thresholds, as gods-eye-view applies them | cited |
| adsb.lol API v2 radius response: `nic`, `nac_p`, `version`, `type`, `seen_pos`, `mlat`, `tisb`, `now` (ms) | `engine.collect.integrity` | inspected 2026-10-29 (live responses) |
| Uber H3 hierarchical hexagonal grid; HydroniumLabs h3o 0.11, a Rust implementation | `kernels/` cells, centres and boundaries | cited; checked against gpsjam.org's cell indices |
| Taiwan Ministry of National Defense, *PLA activities in the waters and airspace around Taiwan*, English list `/en/news/plaactlist/<page>` and reports `/en/News/PLAAct/<n>` | `engine.collect.mnd` | inspected 2026-10-29 (live responses; report 88094) |
| Elie Habib, *World Monitor*, github.com/koala73/worldmonitor, AGPL-3.0, commit 1245ba5, `scripts/cross-strait-activity/adapters.mjs` | where MND publishes and how its reports are worded; no code taken | inspected 2026-10-29 |
| Georgia Tech IODA API v2, `outages/events` (entityType, entityCode, from, until) | `engine.collect.ioda` | inspected 2026-10-29 (live responses; `overall=true` HTTP 500) |
| UCDP, *UCDP Candidate Events Dataset Codebook* v1.5: `date_prec`, `where_prec`, `id` persistence, event definition | `engine.collect.ucdp`, `engine.validate` | inspected 2026-10-29 |
| UCDP downloads page and monthly Candidate CSV `GEDEvent_v26_0_9.csv`; API `ucdpapi.pcr.uu.se` answers 401 without a token | `engine.collect.ucdp` | inspected 2026-10-29 (live responses) |
