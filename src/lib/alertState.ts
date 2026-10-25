import type { AlertEvent, AlertData } from './types/api';

export type AlertPoll = { status: 'ok' | 'unknown'; alerts: AlertEvent[] };
export type StickyAlert = AlertEvent & { lastSeen: number };
export type AlertProviderState = Pick<AlertData, 'status' | 'activeCount' | 'alerts'> & {
  sticky: StickyAlert[];
};

export const STICKY_DURATION = 90_000;

/** Keep resolved alerts briefly, renewing their lifetime while the provider reports them. */
export function advanceAlerts(
  previous: StickyAlert[],
  provider: 'ok' | 'unknown' | 'unavailable',
  incoming: AlertEvent[],
  now: number,
): AlertProviderState {
  if (provider === 'unavailable') {
    return { status: 'UNAVAILABLE', activeCount: 0, alerts: [], sticky: [] };
  }

  const retained = previous.filter(alert => now - alert.lastSeen < STICKY_DURATION);
  if (provider === 'unknown') {
    const alerts = retained.map(publicAlert);
    return { status: 'UNKNOWN', activeCount: 0, alerts, sticky: retained };
  }

  const current = new Map(incoming.map(alert => [identity(alert), alert]));
  const known = new Set<string>();
  const sticky = retained.map(alert => {
    const key = identity(alert);
    const update = current.get(key);
    if (!update) return { ...alert, active: false };
    known.add(key);
    return { ...alert, ...update, id: alert.id, lastSeen: now, active: true };
  });
  for (const alert of current.values()) {
    const key = identity(alert);
    if (!known.has(key)) sticky.push({ ...alert, lastSeen: now, active: true });
  }
  const visible = sticky.filter(alert => now - alert.lastSeen < STICKY_DURATION);
  const activeCount = current.size;
  return {
    status: activeCount ? 'ACTIVE' : 'CLEAR',
    activeCount,
    alerts: visible.map(publicAlert),
    sticky: visible,
  };
}

/** Treat HTTP, JSON, and provider-shape failures as unknown; an empty valid list is clear. */
export async function readAlertProvider(
  fetcher: () => Promise<{ ok: boolean; json: () => Promise<unknown> }>,
  parse: (body: unknown) => AlertEvent[] | Promise<AlertEvent[] | null> | null,
): Promise<AlertPoll> {
  try {
    const response = await fetcher();
    if (!response.ok) return { status: 'unknown', alerts: [] };
    const alerts = await parse(await response.json());
    return Array.isArray(alerts)
      ? { status: 'ok', alerts }
      : { status: 'unknown', alerts: [] };
  } catch {
    return { status: 'unknown', alerts: [] };
  }
}

function identity(alert: AlertEvent) {
  return JSON.stringify([alert.threatOriginal, alert.locationsOriginal]);
}

function publicAlert(alert: StickyAlert): AlertEvent {
  const copy = { ...alert };
  Reflect.deleteProperty(copy, 'lastSeen');
  return copy;
}
