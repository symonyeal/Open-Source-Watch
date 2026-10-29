# Navigation, connectivity and reference signals

The SIGNALS view and the map's GNSS layer add four measurements to each region:
where aircraft report degraded satellite navigation, when a country's internet
connectivity drops, what Taiwan's defence ministry reports about PLA activity,
and the conflict events an academic dataset has coded, against which the engine's
own events are scored. All four are descriptive: none has an alert threshold.

## Sources

| Source | Measure | Collection | Meaning and limits |
| --- | --- | --- | --- |
| [gpsjam.org](https://gpsjam.org/faq) daily files | Aircraft per H3 resolution-4 cell and UTC day reporting good and bad navigation accuracy | Cron, every 12 h; the last 30 days, each day once | Aggregated by gpsjam.org from airplanes.live and ADS-B Exchange. Days the manifest marks `suspect` are flagged incomplete. Cells where few aircraft fly have little data. |
| adsb.lol radius queries (the aircraft feed already fetched) | NIC, NACp and GPS-loss flag of each airborne ADS-B v1/v2 aircraft | Local engine only, every 10 min | Snapshots, not the whole day; the cron does not poll aircraft. |
| [IODA](https://ioda.inetintel.cc.gatech.edu/) `/v2/outages/events` | Country-level outages detected in BGP routing, active probing (ping-slash24) and a network telescope (merit-nt) | Cron, every 6 h; the last 7 days | One event per measurement. The merged `overall=true` events answered HTTP 500 when inspected (2026-10-29). Countries per region are listed in `region.toml` `[outages]`. |
| [Taiwan MND](https://www.mnd.gov.tw/en/news/plaactlist) daily report | Sorties of PLA aircraft, PLAN ships, official ships, balloons; sorties that entered the ADIZ or crossed the median line | Cron, every 12 h; the first poll reads about 90 days of reports | The ministry's own claims, as a party to the dispute. A count the report does not state is missing, not zero. |
| [UCDP Candidate](https://ucdp.uu.se/downloads/) monthly file | Events of organized violence with at least one death: dates, coordinates, their precision, deaths | Cron, weekly; the newest monthly release | Preliminary data; the September 2026 file was published 2026-10-20. The API needs a token; the CSV does not. Events are kept by region map box, so a box can include neighbouring countries. |

## GNSS method

A cell's share is gpsjam.org's formula, `100 × (bad − 1) / (good + bad)`, where
good and bad count the aircraft that flew through the cell in the UTC day. The
first bad aircraft is discounted to limit false positives where few aircraft fly.
Bands follow gpsjam.org: under 2 % low, 2 to 10 % medium, over 10 % high. Medium
and high cells are navigation-integrity anomalies. Jamming or spoofing is one
cause; avionics faults, tests and old transponders are others, so the view never
states jamming as the finding.

For the engine's own snapshots, an aircraft is bad if it reports GPS loss, NACp
below 8, or NIC below 7 (unless NIC is 0 with NACp of 10 or more, which marks an
unknown NIC). These are the US ADS-B Out minima of 14 CFR 91.227(c), applied as
in [gods-eye-view](https://github.com/bilawalsidhu/gods-eye-view); gpsjam.org does
not publish its own per-aircraft rule, and this one is not validated as an
interference classifier. An aircraft counts once per cell and day, as bad if any
of its reports there was bad. Cells are computed by the Rust kernel with
[h3o](https://github.com/HydroniumLabs/h3o), which reproduces all 47,482 cell
indices of gpsjam.org's 2026-10-27 file from their centres.

The view reports how often the two sources agree on cell-days both observed.
That is agreement, not accuracy: the engine samples every ten minutes, gpsjam.org
sees the whole day through more receivers.

## Indicator board

The GNSS cell count and the MND counts (TW-1c) are descriptive indicators. Each
shows its latest value, the latest gpsjam.org day or MND report dated within the
last three days, and is "no data" when there is none, when no source of it is
healthy, or when the latest report does not state the count (an unstated count is
not zero). One rule in `engine/analyze.py` decides this for both.

Each IODA event becomes an `internet_outage` observation placed at the seat of
government. IODA measures a country as one unit, so its observations carry
administrative precision and outages of one country within two hours link into one
event under the fusion rule. The three measurements share one owner, so an event's
credibility does not rise with them. Each region's new PIR counts these events over
48 hours beside its GNSS cell count.

## UCDP reference

UCDP events never become observations; they are the reference. An event is
eligible when its date is exact (`date_prec` 1) and it lies within about 25 km of
its point (`where_prec` 1 or 2), as the Candidate codebook 1.5 defines them, and
when its window falls inside the engine's collection period. It is found if an
engine event of a violent kind (battles, explosions/remote violence, violence
against civilians) starts within 25 km during its local date, or up to one or two
days after for later reporting. All three lags are reported; none is chosen.
`py -3.14 -m engine validate` prints the grid. The share of engine violent events
with a UCDP event in reach is shown too, but it is not precision: UCDP codes only
lethal organized violence.

## Grades

gpsjam.org, IODA, Taiwan MND and UCDP are graded F (cannot be judged) until
assessed, as earlier new sources were. The integrity snapshots carry adsb.lol's B.
IODA outages therefore have credibility 6 and make no truth claim.
