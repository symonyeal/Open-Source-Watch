# Energy, shipping and South Asia

The Energy & Trade view gives economic context for reported disruptions. Select
South Asia in the theater menu to add nine ports and eight countries. The monitor
keeps its incident feeds and reporting thresholds; economic series do not enter
the event classifier or answer an intelligence requirement automatically.

## Connected sources

| Source | Measure | Collection | Meaning and limits |
| --- | --- | --- | --- |
| [EIA Brent via FRED](https://fred.stlouisfed.org/series/DCOILBRENTEU), [WTI](https://fred.stlouisfed.org/series/DCOILWTICO) | Daily spot price, USD/barrel | Every 6 h; latest 90 calendar days | Publication can lag observation. Missing days stay missing. These spot series are separate from Yahoo futures in the monitor. |
| [IMF PortWatch](https://portwatch.imf.org/) | Daily total and tanker port calls / chokepoint transits | Every 6 h; latest 90 calendar days | AIS-derived estimates, published with a lag and subject to revision. Tanker counts do not establish crude cargo, volume or delivery. |
| [World Bank WDI: net energy imports](https://data.worldbank.org/indicator/EG.IMP.CONS.ZS) | Net energy imports as % of energy use | Daily; latest nonmissing year from 2010 onward | Negative values mean net exports. The source is IEA; coverage varies by country and year. |
| [World Bank WDI: fuel imports](https://data.worldbank.org/indicator/TM.VAL.FUEL.ZS.UN) | Fuel imports as % of merchandise imports | Daily; latest nonmissing year from 2010 onward | UN Comtrade / WITS / World Bank estimates. This is a share of import value, not an oil-volume measure. |

The shipping view includes Suez, Bab el-Mandeb, Hormuz and Malacca. South Asia adds
Chattogram, Mongla, Karachi, Port Qasim, Colombo, Mundra, Mumbai/Jawaharlal Nehru,
Yangon and Male. The identifiers were resolved against PortWatch's reference
layer on 2026-10-24; these selected ports do not represent every port in the region.

Country codes are explicitly `BGD, IND, PAK, LKA, MMR, NPL, BTN, MDV`. They do not
come from a provider's regional aggregate. Country searches cover energy,
shipping, security and disasters. RSS coverage uses BBC Asia, Al Jazeera,
gCaptain and Splash247 with a geographic relevance filter. These feeds are
incomplete, English-heavy coverage. Incident indicators show descriptive reporting
counts; they have no local warning/trigger thresholds. Existing classifier accuracy
has not been measured on this theater.
New source reliability is F (cannot be judged) until a source-specific review
supports a grade. Descriptive incident counts include unassessed reports and
do not establish that their claims are true.

## Calculations

- The Brent–WTI spread uses the latest date on which both spot series have a value.
  It never subtracts prices from different dates.
- Shipping uses the centered seven-day mean from the
  [World Bank chokepoints methodology](https://worldbank.github.io/alternative-data-for-crisis/notebooks/disruptions-business-trade/chokepoints-monitor.html).
  This implementation requires complete calendar windows, while the notebook
  allows partial edges. For latest day D, the mean is centered at D−3, covering
  D−6 through D. Any missing measure makes the mean unavailable. Only days before
  today in UTC are used. These averages neither identify a disruption nor its cause.
- Country cells show their own observation years. Unavailable values remain blank;
  they are never replaced by zero or another country's value.
- Every observation shows its date and age. Collection health is separate from
  observation age; an old published observation does not prove a failed collector.

### Recent shipping baseline and release changes

Each shipping series compares its latest complete seven-day mean with the median
of the four immediately preceding, nonoverlapping seven-day means. All 35 calendar
days must have the measure. The response includes each window and its mean, the
number of complete baseline weeks, the baseline median and the percentage change.
A missing week makes the comparison unavailable; a zero baseline makes its percentage
change unavailable. Total and tanker counts are calculated separately.

This project's rolling four-week reference adapts to recent activity. It is a
descriptive extension of the seven-day smoothing above. The World Bank study used
a fixed historical reference for its particular crisis; this rolling reference
does not estimate seasonal normality, predict a disruption, identify its cause or
set an alert threshold. Sustained disruption will eventually enter the reference.

Each series also identifies its raw release and compares the current release with
the preceding fetched release. `revision.changes` lists changed values for country/date
keys appearing in both, including withdrawn and restored values. New dates and dates
that leave the 90-day retrieval window are excluded from revision counts. A revision
count describes provider changes, not estimation error. Historical views select only
releases fetched by the requested time, so a later correction cannot alter an
earlier as-of answer. With no previous recorded release, revision information is
unavailable.

The [maritime methods review](MARITIME-METHODS.md) covers the supplied satellite
dataset catalog, iMoonLab repositories, established SAR processing references
and evaluation requirements. No SAR ship detector is installed or represented
as a live data source.

## Recording and access

`engine/src/collect/context.rs` makes bounded public requests. A successful poll writes
an immutable series snapshot to `raw_report`; `engine/src/context.rs` reads the latest
release of each series. This keeps corrections and reversions in their retrieval
order. A truncated provider response or an API error is a collection failure.
No new dependencies are needed.

Parser acceptance rejects duplicate country/date keys, invalid dates, unknown context
types, negative or fractional ship counts, and tanker counts above total counts.
Negative prices and net-energy-export shares remain valid. Provider nulls remain
missing. A successful parse does not validate the provider's underlying AIS coverage.

Polls stage immutable Parquet files and publish their related tables with one
generation manifest. Requests pin that manifest; compaction retains older files.
An OS file lock rejects independent writers to the same root. See
[integrity and recovery](ENGINE-INTEGRITY.md) for migration and retention limits.

`GET /api/context?region=south-asia` on the engine returns the stored
series, aligned spread, country scope and source health. Next.js forwards this
view through `fromEngine`. With no engine available, it returns HTTP 503 with an
unavailable message. It does not fetch a second, unrecorded copy of these data.

The collection workflow writes context snapshots to the existing `data` branch.
To use that history locally, check out that branch at `./data` as described in
the working contract. The dashboard does not download a Git branch automatically.

## Attribution and reuse

Credit oil data to the U.S. Energy Information Administration, retrieved from
FRED, Federal Reserve Bank of St. Louis, with the series link. These series are
tagged public domain with citation requested; FRED graph download URLs are export
endpoints with no API availability guarantee.

Shipping attribution: **Sources: UN Global Platform; IMF PortWatch
([portwatch.imf.org](https://portwatch.imf.org/))**. The rolling averages and percent
changes are calculations of this project. [IMF statistical-data terms](https://www.imf.org/en/About/copyright-and-terms)
allow accurate attributed reuse and require disclosure of material transformations;
they request contact for potential commercial reuse. Upstream third-party rights
continue to apply. This repository supplies no commercial redistribution licence.

The two World Bank indicator pages specify CC BY 4.0. Retain World Bank and
underlying IEA / UN attribution. Their annual values provide structural context;
they do not measure current inventories, import bills or reserve adequacy.

## Further sources assessed

[EIA Open Data](https://www.eia.gov/opendata/documentation.php) can add weekly
stocks, refinery runs and import/export flows after a free key is configured.
[World Bank Pink Sheet](https://www.worldbank.org/en/research/commodity-markets)
can add monthly Dubai/Brent/WTI baselines. [USGS](https://earthquake.usgs.gov/fdsnws/event/1/)
and [GDACS](https://www.gdacs.org/) can add dated disaster observations after
country and coastline filtering is validated. These are research candidates,
not connected feeds. NOAA's historical AIS archive is U.S.-focused and does not
fill South Asian live AIS gaps.
