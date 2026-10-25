# Source and correctness review

Readings and checks from 2026-10-24. “Python” means executed evidence from the
collector; “Cited” means read from the linked source. A live response establishes
that request's result, not continued availability or complete coverage.

| Identification or reading | Basis | Status |
| --- | --- | --- |
| Existing market panel contains Yahoo futures and existing AIS requires a key | `engine/collect/markets.py`, `ais.py`, source and decision records | Cited |
| FRED oil series are EIA-origin daily spot in USD/barrel | [Brent](https://fred.stlouisfed.org/series/DCOILBRENTEU), [WTI](https://fred.stlouisfed.org/series/DCOILWTICO); separate CSV requests returned HTTP 200 | Python / Cited |
| PortWatch provides public daily total/tanker counts with no token | [Chokepoint metadata](https://services9.arcgis.com/weJ1QsnbMYJlCHdG/arcgis/rest/services/Daily_Chokepoints_Data/FeatureServer/0?f=pjson), [port metadata](https://services9.arcgis.com/weJ1QsnbMYJlCHdG/arcgis/rest/services/Daily_Ports_Data/FeatureServer/0?f=pjson); bounded data queries HTTP 200 | Python / Cited |
| Current PortWatch dates are ISO date strings; older clients used epoch milliseconds | Live bounded query versus [World Bank methodology example](https://worldbank.github.io/alternative-data-for-crisis/notebooks/disruptions-business-trade/chokepoints-monitor.html) | Python / Corrected |
| South Asian port identifiers and four chokepoint identifiers resolve to the stated locations | [Port reference layer](https://services9.arcgis.com/weJ1QsnbMYJlCHdG/arcgis/rest/services/PortWatch_ports_database/FeatureServer/0?f=pjson), public reference queries | Python |
| Tanker activity is a vessel-class proxy; it does not establish crude deliveries | [IMF methodology paper](https://www.imf.org/-/media/files/publications/wp/2025/english/wpiea2025093-print-pdf.pdf) | Cited |
| Attributed statistical-data reuse has terms distinct from general IMF content | [IMF terms](https://www.imf.org/en/About/copyright-and-terms), statistical-data section | Cited |
| World Bank API accepts explicit countries and returns nullable observations by country/year | [API documentation](https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures); two live eight-country responses | Python / Cited |
| Energy and fuel indicators measure different denominators and permit incomplete country coverage | [Net energy imports](https://data.worldbank.org/indicator/EG.IMP.CONS.ZS), [fuel imports](https://data.worldbank.org/indicator/TM.VAL.FUEL.ZS.UN); live collector retained 6/8 and 8/8 nonmissing country values respectively | Python / Cited |
| All 17 new sources reached the recorded view | `engine collect --adapter context`: 17 ok, 17 snapshots; context view read back 17 series | Python |
| Poll cadence was wrongly shared between regions; missing evidence was counted as absent | Failing then passing tests in `tests/test_regressions.py` | Python / Corrected |
| Daily baselines could be hidden after 24 hours; constant AIS baselines could break JSON | Failing then passing health and serialization regressions | Python / Corrected |
| Product confidence used the product window instead of the indicator window | Failing then passing 72-hour maritime / 6-hour SITREP regression | Python / Corrected |
| A reader could encounter an unfinished Parquet poll | Opening an empty destination during a write made a query fail before atomic file publication; the regression passes after the change | Python / Corrected |
| South Asia needed additional place names | Rebuilt the public GeoNames cities15000 lookup: 11,353 names; Khulna, Lahore and Mandalay geocoding regressions | Python |
| Google News queries can include unrelated places and entertainment | Live South Asia poll: 354 raw / 303 observations, including Thailand, Paris and film examples; scoped-title regression tests | Python / Corrected |
| Geographic filtering changes the accepted headline set | Replay of the same 354 raw reports produced 249 observations: 58 old observations removed and four RSS observations added through demonyms; headlines that imply a country without naming it can be lost | Python; precision/recall not measured |
| Yahoo daily metadata can omit `previousClose` | Live CL=F HTTP 200; fallback uses the prior completed daily bar, instead of the first close in a five-day range | Python / Corrected |
| Pink Sheet is monthly; EIA API needs a key; GDACS and USGS are possible disaster feeds | [Pink Sheet](https://www.worldbank.org/en/research/commodity-markets), [EIA](https://www.eia.gov/opendata/documentation.php), [GDACS](https://www.gdacs.org/), [USGS](https://earthquake.usgs.gov/fdsnws/event/1/) | Cited; not integrated |
| European grid data and U.S. AIS archives do not answer the South Asian oil/shipping requirement | [Energy-Charts](https://api.energy-charts.info/), [NOAA AIS](https://coast.noaa.gov/digitalcoast/tools/ais.html) coverage descriptions | Cited; excluded |
| Supplied ship repository is a dataset index, with per-dataset terms | [Catalog](https://github.com/jasonmanesis/Satellite-Imagery-Datasets-Containing-Ships); followed HRSID and xView3 links to their authors | Cited |
| HRSID is a ship detection/segmentation benchmark | [Authors' README](https://github.com/chaozhong2010/HRSID), including background-only test imagery | Cited; not run |
| xView3 supplies a reference pipeline and evaluation settings | [README and scorer](https://github.com/DIUx-xView/xview3-reference), [paper](https://arxiv.org/abs/2206.00897), BSD-3-Clause code licence | Cited; not run |
| iMoonLab has 31 public repositories; relevant detector results use COCO | Public GitHub API inventory; [YOLOv13](https://github.com/iMoonLab/yolov13), [Hyper-YOLO](https://github.com/iMoonLab/Hyper-YOLO), DeepHypergraph and Hypergraph-DB READMEs | Cited; no maritime performance measured |
| YOLOv13's supplied requirements do not match this host | Torch 2.2.2 and CPython 3.11 Linux wheel in [requirements](https://github.com/iMoonLab/yolov13/blob/main/requirements.txt); host Windows/Python 3.14 | Cited; no installation attempted |
| Existing SAR processing and imagery discovery references are available | [ESA SNAP ship-detection training](https://eo4society.esa.int/event/rus-remote-training-ship-detection-with-sentinel-1-using-snap-s-1-toolbox/), [Copernicus STAC](https://documentation.dataspace.copernicus.eu/APIs/STAC.html) | Cited; not connected |
| Shipping smoothing follows a published method | [World Bank notebook](https://worldbank.github.io/alternative-data-for-crisis/notebooks/disruptions-business-trade/chokepoints-monitor.html), centered 7-day rolling mean; full calendar windows required here | Cited / Python |
| South Asia has no evidence-based alert cutoffs | Removed the uncalibrated new watch/trigger counts; indicators remain descriptive and cannot answer PIRs | Corrected |
| New sources have no reliability assessment | New context providers and South Asia publishers receive F (cannot be judged); reporting counts include credibility 6 and make no truth claim | Corrected |
