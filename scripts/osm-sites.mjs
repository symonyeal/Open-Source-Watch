// Write regions/sites.tsv, the sites engine/src/process/place.rs matches: refineries,
// oil depots, airfields, power plants, substations, ports, bridges, dams and plants in
// lat 41-70° N, lon 19-60° E (Ukraine, European Russia and their neighbours), from
// OpenStreetMap through the Overpass API
// (data © OpenStreetMap contributors, ODbL 1.0; docs/SOURCES.md). One Overpass query
// per kind; each answer is kept in <cache dir>/<kind>.json and reused while present.
// Run: node scripts/osm-sites.mjs <cache dir>
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const URL = 'https://overpass-api.de/api/interpreter';
const UA = 'Open-Source-Watch/1.0 (github.com/symonyeal/Open-Source-Watch)';
const BOX = '(41,19,70,60)';
const REF = 'НПЗ|НПК|НОРСИ|[Нн]ефтеперераб|[Нн]афтоперероб|[Нн]ефтепродукт|[Rr]efinery';
const DEPOT = '[Нн]ефтебаз|[Нн]афтобаз|[Нн]ефтехранил|[Оо]il depot|[Пп]ерекачивающ|ЛПДС|НПС';
// Tag filters by kind (OSM wiki: landuse, man_made, aeroway, military, power, harbour,
// waterway); bridges, dams, harbours and works only with a Wikidata item.
const KINDS = {
  refinery: [
    `nwr["landuse"="industrial"]["name"~"${REF}"]`,
    `nwr["man_made"="works"]["name"~"${REF}"]`,
    'nwr["industrial"~"^(refinery|oil_refinery)$"]',
  ],
  oil_depot: [
    'nwr["industrial"="oil"]["name"]',
    `nwr["landuse"="industrial"]["name"~"${DEPOT}"]`,
    `nwr["man_made"="works"]["name"~"${DEPOT}"]`,
  ],
  airfield: [
    'nwr["aeroway"="aerodrome"]["military"]["name"]',
    'nwr["aeroway"="aerodrome"]["icao"]["name"]',
    'nwr["military"="airfield"]["name"]',
  ],
  power_plant: ['nwr["power"="plant"]["name"]["plant:source"!~"solar|wind"]'],
  substation: ['nwr["power"="substation"]["name"]["voltage"~"(220|330|400|500|750)000"]'],
  port: [
    'nwr["landuse"="port"]["name"]',
    'nwr["industrial"="port"]["name"]',
    'nwr["harbour"="yes"]["name"]["wikidata"]',
  ],
  bridge: ['nwr["man_made"="bridge"]["name"]["wikidata"]'],
  dam: ['nwr["waterway"="dam"]["name"]["wikidata"]'],
  plant: ['nwr["man_made"="works"]["name"]["wikidata"]'],
};

// The box only, not the country areas: an area query over Russia timed out (2026-11-02),
// and a site is used only near a town the report names (process::place).
const query = (F) => `[out:json][timeout:600];(${F.map((f) => `${f}${BOX};`).join('')});out center tags;`;

async function ask(q) {
  for (let k = 1; k <= 8; k++) {
    const r = await fetch(URL, {
      method: 'POST',
      headers: { 'User-Agent': UA, 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ data: q }),
    }).catch((e) => ({ ok: false, status: String(e) }));
    // A query that runs out of time or memory answers 200 with a partial result and a remark.
    const j = r.ok ? await r.json().catch(() => null) : null;
    if (j && !/error/i.test(j.remark ?? '')) return j;
    console.error(`overpass ${r.status} ${j?.remark ?? ''}; retry ${k}`);
    await new Promise((f) => setTimeout(f, 30000 * k));
  }
  throw new Error('Overpass did not answer');
}

// Cyrillic to Latin: Ukrainian national transliteration (2010) for a name with і ї є ґ,
// else BGN/PCGN Russian, simplified (no diacritics).
const UK = { г: 'h', и: 'y', і: 'i', ї: 'i', є: 'ie', ґ: 'g', й: 'i', х: 'kh', щ: 'shch' };
const RU = {
  а: 'a', б: 'b', в: 'v', г: 'g', д: 'd', е: 'e', ё: 'e', ж: 'zh', з: 'z', и: 'i',
  й: 'y', к: 'k', л: 'l', м: 'm', н: 'n', о: 'o', п: 'p', р: 'r', с: 's', т: 't',
  у: 'u', ф: 'f', х: 'kh', ц: 'ts', ч: 'ch', ш: 'sh', щ: 'shch', ъ: '', ы: 'y',
  ь: '', э: 'e', ю: 'yu', я: 'ya',
};
function latin(s) {
  const T = /[іїєґІЇЄҐ]/.test(s) ? { ...RU, ...UK, ь: '', ю: 'iu', я: 'ia' } : RU;
  return [...s].map((c) => {
    const l = c.toLowerCase();
    if (!(l in T)) return c;
    const x = T[l];
    return c === l ? x : x.charAt(0).toUpperCase() + x.slice(1);
  }).join('');
}

const clean = (s) => s.replace(/[«»"“”„]/g, '').replace(/\s+/g, ' ').trim();
const dir = process.argv[2];
if (!dir) throw new Error('usage: node scripts/osm-sites.mjs <cache dir>');
mkdirSync(dir, { recursive: true });
const rows = new Map();
for (const [kind, F] of Object.entries(KINDS)) {
  const p = `${dir}/${kind}.json`;
  if (!existsSync(p)) writeFileSync(p, JSON.stringify(await ask(query(F))));
  for (const e of JSON.parse(readFileSync(p, 'utf8')).elements) {
    const t = e.tags ?? {};
    const g = e.center ?? e;
    const raw = t['name:en'] ?? t.int_name ?? t.name;
    if (!raw || g.lat === undefined) continue;
    const name = clean(latin(raw));
    // A nuclear plant is its own kind: it is named after a town far from it.
    const k = kind === 'power_plant' && t['plant:source'] === 'nuclear' ? 'nuclear_plant' : kind;
    // One row per kind and name within 0.01° (an object mapped as node and area).
    const key = `${k}\t${name}\t${g.lat.toFixed(2)}\t${g.lon.toFixed(2)}`;
    if (!rows.has(key)) rows.set(key, [name, g.lat.toFixed(5), g.lon.toFixed(5), k, `${e.type[0]}${e.id}`]);
  }
}
const body = [...rows.values()].sort((a, b) => (a[3] + a[0] + a[4]).localeCompare(b[3] + b[0] + b[4]));
writeFileSync('regions/sites.tsv', ['name\tlat\tlon\tkind\tosm', ...body.map((r) => r.join('\t'))].join('\n') + '\n');
console.log(`${body.length} sites written`);
