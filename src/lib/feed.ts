export type FeedSnapshot<T> = {
  url: string;
  data: T | null;
  loading: boolean;
  error: string | null;
  lastUpdated: Date | null;
};

export function feedSuccess<T>(url: string, data: T, time: Date): FeedSnapshot<T> {
  return { url, data, loading: false, error: null, lastUpdated: time };
}

export function feedFailure<T>(
  previous: FeedSnapshot<T>, url: string, error: string,
): FeedSnapshot<T> {
  const current: FeedSnapshot<T> = previous.url === url
    ? previous
    : { url, data: null, loading: true, error: null, lastUpdated: null };
  return { ...current, loading: false, error };
}

/** Poll one URL until stopped; ignore responses from a stopped URL lifecycle. */
export function startFeed<T>(
  url: string,
  interval: number,
  onData: (data: T, time: Date) => void,
  onError: (message: string) => void,
  fetcher: typeof fetch = fetch,
  schedule: (callback: () => void, interval: number) => () => void = (callback, ms) => {
    const id = setInterval(callback, ms);
    return () => clearInterval(id);
  },
) {
  let active = true;
  let inFlight = false;
  let controller: AbortController | null = null;

  const refresh = async () => {
    if (!active || inFlight) return;
    inFlight = true;
    const request = new AbortController();
    controller = request;
    try {
      const bustUrl = `${url}${url.includes('?') ? '&' : '?'}_t=${Date.now()}`;
      const res = await fetcher(bustUrl, { cache: 'no-store', signal: request.signal });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = await res.json() as T;
      if (!active || controller !== request) return;
      onData(json, new Date());
    } catch (err) {
      if (active && controller === request && !request.signal.aborted) {
        onError(err instanceof Error ? err.message : 'Failed to fetch');
      }
    } finally {
      if (controller === request) {
        inFlight = false;
        controller = null;
      }
    }
  };

  void refresh();
  const cancel = schedule(() => void refresh(), interval);
  return {
    refresh,
    stop() {
      active = false;
      cancel();
      controller?.abort();
    },
  };
}

/** Hide a prior URL's result immediately while the next effect starts. */
export function visibleFeed<T>(url: string, snapshot: FeedSnapshot<T>, initialData: T | null) {
  return snapshot.url === url
    ? snapshot
    : { url, data: initialData, loading: true, error: null, lastUpdated: null };
}
