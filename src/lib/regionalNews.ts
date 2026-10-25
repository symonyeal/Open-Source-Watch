import type { ServerConfig } from '@/lib/conflicts/types';

const SOUTH_ASIA_NOISE = /\b(?:cricket(?:ers?)?|ipl|football|soccer|sports?|film|movies?|cinema|actors?|actresses?|bollywood|hollywood|tollywood|box.?office|trailers?|celebrities|entertainment|streaming|netflix|kartik aaryan|world.?cup|fifa|olympics?|premier.?league|champions.?league|super.?bowl|nba|nfl|nhl|mlb|grammy|oscar|emmy|eurovision)\b/i;

const COUNTRY_ALIASES: Record<string, string[]> = {
  bangladesh: ['Bangladesh', 'Bangladeshi'],
  india: ['India', 'Indian'],
  pakistan: ['Pakistan', 'Pakistani'],
  'sri lanka': ['Sri Lanka', 'Sri Lankan', 'Ceylon', 'Ceylonese'],
  myanmar: ['Myanmar', 'Burma', 'Burmese'],
  nepal: ['Nepal', 'Nepali', 'Nepalese'],
  bhutan: ['Bhutan', 'Bhutanese'],
  maldives: ['Maldives', 'Maldivian'],
};

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function mentionsCountry(title: string, country: string) {
  const aliases = COUNTRY_ALIASES[country.toLocaleLowerCase()] ?? [country];
  return aliases.some(alias => new RegExp(
    `(^|[^\\p{L}\\p{N}])${escapeRegExp(alias)}(?=$|[^\\p{L}\\p{N}])`, 'iu',
  ).test(title));
}

/** Keep South Asia Google News results geographically relevant. A country
 * query must name that country or one of its recognized English aliases.
 */
export function acceptsGoogleNewsTitle(
  title: string,
  server: Pick<ServerConfig, 'filterGoogleNews' | 'newsRelevanceKeywords'>,
  country?: string,
) {
  if (!server.filterGoogleNews) return true;
  if (SOUTH_ASIA_NOISE.test(title)) return false;

  const geography = server.newsRelevanceKeywords;
  const previousIndex = geography.lastIndex;
  geography.lastIndex = 0;
  const regionMatch = geography.test(title);
  geography.lastIndex = previousIndex;
  if (!regionMatch) return false;

  return country ? mentionsCountry(title, country) : true;
}
