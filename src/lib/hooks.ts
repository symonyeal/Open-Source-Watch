import { useState, useEffect, useCallback, useRef } from 'react';
import { useConflict } from './conflicts/context';
import { feedFailure, feedSuccess, startFeed, visibleFeed, type FeedSnapshot } from './feed';
import { apiUrl } from './snapshot';

export function useDataFeed<T>(url: string, interval: number = 60000, initialData: T | null = null) {
  const [snapshot, setSnapshot] = useState<FeedSnapshot<T>>({
    url, data: initialData, loading: true, error: null, lastUpdated: null,
  });
  const poller = useRef<{ refresh: () => Promise<void> } | null>(null);

  useEffect(() => {
    const active = startFeed<T>(
      apiUrl(url),
      interval,
      (data, time) => setSnapshot(feedSuccess(url, data, time)),
      error => setSnapshot(current => feedFailure(current, url, error)),
    );
    poller.current = active;
    return () => {
      active.stop();
      if (poller.current === active) poller.current = null;
    };
  }, [url, interval]);

  const refetch = useCallback(() => poller.current?.refresh(), []);
  return { ...visibleFeed(url, snapshot, initialData), refetch };
}

/**
 * Like useDataFeed, but automatically appends the active conflict as a
 * `?conflict=<key>` query param. When the user toggles conflicts the URL
 * changes, so the feed re-fetches for the newly selected theater.
 */
export function useConflictFeed<T>(path: string, interval: number = 60000, initialData: T | null = null) {
  const { key } = useConflict();
  const url = `${path}${path.includes('?') ? '&' : '?'}conflict=${key}`;
  return useDataFeed<T>(url, interval, initialData);
}

/** Forces a re-render every `ms` milliseconds so relative timestamps stay fresh. */
export function useTick(ms: number = 15000) {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick(t => t + 1), ms);
    return () => clearInterval(id);
  }, [ms]);
}

export function timeAgo(date: string | Date): string {
  if (!date) return '';
  const now = new Date();
  const then = new Date(date);

  // Invalid date
  if (isNaN(then.getTime())) return '';

  const seconds = Math.floor((now.getTime() - then.getTime()) / 1000);
  const abs = Math.abs(seconds);

  if (abs < 60) return 'just now';
  if (abs < 3600) return `${Math.floor(abs / 60)}m ago`;
  if (abs < 86400) return `${Math.floor(abs / 3600)}h ago`;
  return `${Math.floor(abs / 86400)}d ago`;
}

export function formatPrice(price: number, decimals: number = 2): string {
  return price.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

export function formatChange(change: number, percent: number): string {
  const c = change ?? 0;
  const p = percent ?? 0;
  const sign = c >= 0 ? '+' : '';
  return `${sign}${c.toFixed(2)} (${sign}${p.toFixed(2)}%)`;
}
