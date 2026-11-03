import { NextResponse } from 'next/server';
import { translateFreeText } from '@/lib/hebrew';
import { getConflictFromRequest } from '@/lib/conflicts';
import { extractTelegramText } from '@/lib/telegramText';
import { fromEngine } from '@/lib/engine';
import type { TelegramPost } from '@/lib/types/api';

// Detect non-Latin scripts (Hebrew, Arabic, Farsi, Cyrillic, etc.)
function hasNonLatinText(text: string): boolean {
  return /[\u0590-\u05FF\u0600-\u06FF\u0750-\u077F\uFB50-\uFDFF\uFE70-\uFEFF\u0400-\u04FF]/.test(text);
}

export const dynamic = 'force-dynamic';

// Scrape public Telegram channels via embed endpoint
// Completely free, no API key, no bot needed
// Channel list is per-conflict (see src/lib/conflicts/*).

// Persist latest known post IDs across requests (in-memory cache)
const latestKnownIds: Record<string, number> = {};
// Cache of fetched posts so we don't re-fetch. Bounded: oldest entries are
// evicted first (Map keeps insertion order) so a long-running container
// does not grow without limit.
const POST_CACHE_MAX = 5000;
const postCache = new Map<string, { text: string; date: string }>();
// Probes that found no post, until their expiry (ms): a post id not yet used is asked
// again only after MISS_TTL, and each poll reuses one response for RESPONSE_TTL.
const MISS_TTL = 60_000;
const RESPONSE_TTL = 45_000;
const missCache = new Map<string, number>();
const responseCache = new Map<string, { at: number; body: unknown }>();

function evictOldest<K, V>(m: Map<K, V>) {
  if (m.size > POST_CACHE_MAX) {
    const oldest = m.keys().next().value;
    if (oldest !== undefined) m.delete(oldest);
  }
}

async function fetchPost(channel: string, postId: number): Promise<{ text: string; date: string } | null> {
  const cacheKey = `${channel}/${postId}`;
  const cached = postCache.get(cacheKey);
  if (cached) return cached;
  if ((missCache.get(cacheKey) ?? 0) > Date.now()) return null;
  const miss = () => {
    missCache.delete(cacheKey);
    missCache.set(cacheKey, Date.now() + MISS_TTL);
    evictOldest(missCache);
    return null;
  };

  try {
    const res = await fetch(`https://t.me/${channel}/${postId}?embed=1&mode=tme`, {
      signal: AbortSignal.timeout(3000),
      headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' },
    });

    if (!res.ok) return miss();
    const html = await res.text();

    const extracted = extractTelegramText(html);
    if (!extracted) return miss();
    let text = extracted;

    const dateMatch = html.match(/<time[^>]*datetime="([^"]+)"/);
    const date = dateMatch ? dateMatch[1] : new Date().toISOString();

    // Auto-translate non-Latin text (Hebrew, Farsi, Arabic, etc.)
    if (hasNonLatinText(text)) {
      text = await translateFreeText(text);
    }

    const result = { text, date };
    postCache.set(cacheKey, result);
    evictOldest(postCache);
    return result;
  } catch {
    return null;
  }
}

// On first call, find latest post via binary search. After that, just check ahead.
async function findLatestPostId(channel: string): Promise<number> {
  const known = latestKnownIds[channel];

  if (known) {
    // Check 5 ahead in parallel; while the newest of them exists, check 5 more
    // (at most 20 ahead in all, as before).
    let highest = known;
    for (let round = 0; round < 4; round++) {
      const base = highest;
      const checks = Array.from({ length: 5 }, (_, i) => base + 5 - i);
      const results = await Promise.allSettled(
        checks.map(id => fetchPost(channel, id).then(r => r ? id : null))
      );
      for (const r of results) {
        if (r.status === 'fulfilled' && r.value && r.value > highest) {
          highest = r.value;
        }
      }
      if (highest < base + 5) break;
    }
    latestKnownIds[channel] = highest;
    return highest;
  }

  // First time: binary search (sequential but fast with big jumps)
  let low = 1;
  let high = 200000;

  // Quick probe to find rough range
  for (const probe of [500, 5000, 15000, 30000, 50000, 80000, 120000, 180000]) {
    if (probe >= high) break;
    const result = await fetchPost(channel, probe);
    if (result) {
      low = probe;
    } else {
      high = probe;
      break;
    }
  }

  // Binary search
  while (high - low > 10) {
    const mid = Math.floor((low + high) / 2);
    const result = await fetchPost(channel, mid);
    if (result) {
      low = mid;
    } else {
      high = mid;
    }
  }

  // Fine scan the last few
  for (let i = high; i >= low; i--) {
    const result = await fetchPost(channel, i);
    if (result) {
      latestKnownIds[channel] = i;
      return i;
    }
  }

  latestKnownIds[channel] = low;
  return low;
}

export async function GET(req: Request) {
  const engine = await fromEngine(req);
  if (engine) return engine;
  const { key, server } = getConflictFromRequest(req);
  const channels = server.telegramChannels;
  const memo = responseCache.get(key);
  if (memo && Date.now() - memo.at < RESPONSE_TTL) {
    return NextResponse.json(memo.body, {
      headers: { 'Cache-Control': 'no-cache, no-store, must-revalidate' },
    });
  }

  // Process ALL channels in parallel — each finds latest + fetches 3 posts
  const channelResults = await Promise.allSettled(
    channels.map(async (channel) => {
      const latestId = await findLatestPostId(channel.name);
      const posts: TelegramPost[] = [];

      // Fetch only latest 3 posts in parallel
      const ids = [latestId, latestId - 1, latestId - 2].filter(id => id > 0);
      const results = await Promise.allSettled(
        ids.map(id => fetchPost(channel.name, id))
      );

      results.forEach((r, i) => {
        if (r.status === 'fulfilled' && r.value) {
          posts.push({
            channel: channel.name,
            channelLabel: channel.label,
            color: channel.color,
            postId: ids[i],
            text: r.value.text,
            date: r.value.date,
            url: `https://t.me/${channel.name}/${ids[i]}`,
          });
        }
      });

      return posts;
    })
  );

  const allPosts: TelegramPost[] = [];
  for (const result of channelResults) {
    if (result.status === 'fulfilled') {
      allPosts.push(...result.value);
    }
  }

  // Sort newest first
  allPosts.sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());

  const body = {
    posts: allPosts,
    channels: channels.map(c => c.label),
    updated: new Date().toISOString(),
  };
  responseCache.set(key, { at: Date.now(), body });
  return NextResponse.json(body, {
    headers: { 'Cache-Control': 'no-cache, no-store, must-revalidate' },
  });
}
