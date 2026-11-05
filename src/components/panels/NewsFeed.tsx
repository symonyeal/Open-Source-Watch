'use client';

import { useConflictFeed, timeAgo, useTick } from '@/lib/hooks';
import { useConflict } from '@/lib/conflicts/context';
import type { NewsItem } from '@/types';

export default function NewsFeed() {
  const { config } = useConflict();
  const SOURCE_COLORS = config.client.sourceColors;
  const { data: news, loading, error } = useConflictFeed<NewsItem[]>('/api/news', 90000);
  useTick(15000);

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        News reports
        <span className="ml-auto text-[9px] text-[var(--text-secondary)] font-normal normal-case tracking-normal">
          {news?.length || 0} items{news?.some(item => item.raw_id) ? ' · recorded reports' : ' · recording not established'}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="space-y-2 p-3">
            {[...Array(8)].map((_, i) => (
              <div key={i} className="loading-shimmer h-12 rounded" />
            ))}
          </div>
        ) : error ? <p className="p-3 text-sm" role="status">Reports unavailable: {error}</p> : !news?.length ? <p className="p-3 text-sm">No recorded reports in this window.</p> : (
          news?.map((item, i) => (
            <a
              key={i}
              href={item.link}
              target="_blank"
              rel="noopener noreferrer"
              className="data-row flex items-start gap-2 hover:cursor-pointer block"
            >
              <span
                className="text-[9px] font-bold px-1.5 py-0.5 rounded mt-0.5 shrink-0"
                style={{
                  backgroundColor: SOURCE_COLORS[item.source] || '#555',
                  color: '#fff',
                }}
              >
                {item.source}
              </span>
              <div className="min-w-0 flex-1">
                <p className="text-sm leading-snug text-[var(--text-primary)]">
                  {item.title}
                </p>
                <span className="text-[9px] text-[var(--text-secondary)]">
                  {item.pubDate ? `Published ${timeAgo(item.pubDate)}` : item.fetched_at ? `Collected ${new Date(item.fetched_at).toISOString()} · publication time not qualified` : 'Publication time unavailable'}
                </span>
              </div>
            </a>
          ))
        )}
      </div>
    </div>
  );
}
