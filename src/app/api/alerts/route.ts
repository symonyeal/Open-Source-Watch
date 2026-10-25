import { NextResponse } from 'next/server';

import { fetchWithTimeout } from '@/lib/fetcher';
import { translateHebrew, translateCities, isHebrew, translateFreeText, CITY_TRANSLATIONS } from '@/lib/hebrew';
import { getConflictFromRequest } from '@/lib/conflicts';
import type { AlertEvent } from '@/lib/types/api';
import { advanceAlerts, readAlertProvider, type StickyAlert } from '@/lib/alertState';

export const dynamic = 'force-dynamic';

// Alerts remain visible for 90 seconds after their last observation.
const stickyByConflict: Record<string, StickyAlert[]> = {};

export async function GET(req: Request) {
  const { key, client, server } = getConflictFromRequest(req);
  const sourceLabel = client.alertSystemName;
  const poll = server.alertProvider === 'tzevaadom'
    ? await fetchTzeAdomAlerts(sourceLabel)
    : server.alertProvider === 'alertsua'
      ? await fetchUkraineAlerts(sourceLabel)
      : null;
  const provider = server.alertProvider === 'none' || !poll ? 'unavailable' : poll.status;
  const now = Date.now();
  const state = advanceAlerts(stickyByConflict[key] || [], provider, poll?.alerts || [], now);
  stickyByConflict[key] = state.sticky;

  return NextResponse.json({
    status: state.status,
    activeCount: state.activeCount,
    alerts: state.alerts,
    lastChecked: new Date(now).toISOString(),
    source: sourceLabel,
  }, {
    headers: { 'Cache-Control': 'public, s-maxage=5, stale-while-revalidate=3' },
  });
}

async function fetchTzeAdomAlerts(sourceLabel: string) {
  return readAlertProvider(
    () => fetchWithTimeout('https://api.tzevaadom.co.il/notifications', {
      timeout: 12000,
      headers: { 'User-Agent': 'IronSight/1.0', 'Accept': 'application/json' },
    }),
    async body => {
      if (!Array.isArray(body) || !body.every(isTzeAdomAlert)) return null;
      const alerts: AlertEvent[] = body.map((alert: TzevaAdomAlert, i: number) => {
        const rawThreat = alert.threat || alert.title || 'Alert';
        const rawCities = Array.isArray(alert.cities) ? alert.cities : [alert.data || 'Unknown'];

        let translatedThreat = translateHebrew(rawThreat);
        const translatedLocations = translateCities(rawCities);
        if (CITY_TRANSLATIONS[rawThreat]) {
          if (!rawCities.includes(rawThreat)) translatedLocations.push(CITY_TRANSLATIONS[rawThreat]);
          translatedThreat = 'Rocket/Missile Alert';
        }
        return {
          id: `tzeva-${i}-${Date.now()}`,
          time: alert.date || new Date().toISOString(),
          type: categorizeAlert(rawThreat),
          threat: translatedThreat,
          threatOriginal: rawThreat,
          locations: translatedLocations,
          locationsOriginal: rawCities,
          source: sourceLabel,
          active: true,
        };
      });
      await Promise.all(alerts.map(async alert => {
        if (isHebrew(alert.threat)) alert.threat = await translateFreeText(alert.threat);
        alert.locations = await Promise.all(
          alert.locations.map(loc => isHebrew(loc) ? translateFreeText(loc) : Promise.resolve(loc))
        );
      }));
      return alerts;
    },
  );
}

async function fetchUkraineAlerts(sourceLabel: string) {
  return readAlertProvider(
    () => fetchWithTimeout('https://alerts.com.ua/api/states', {
      timeout: 12000,
      headers: { 'User-Agent': 'IronSight/1.0', 'Accept': 'application/json' },
    }),
    body => {
      if (!isRecord(body) || !Array.isArray(body.states) || !body.states.every(isAlertsUaState)) {
        return null;
      }
      return body.states.filter((s: AlertsUaState) => s.alert).map((s: AlertsUaState, i: number) => {
        const name = s.name_en || s.name || 'Unknown';
        return {
          id: `ua-${s.id ?? i}-${Date.now()}`,
          time: s.changed || new Date().toISOString(),
          type: 'ALERT',
          threat: 'Air Raid Alert',
          threatOriginal: `ua-${s.id ?? name}`,
          locations: [name],
          locationsOriginal: [name],
          source: sourceLabel,
          active: true,
        };
      });
    },
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function isTzeAdomAlert(value: unknown): value is TzevaAdomAlert {
  if (!isRecord(value)) return false;
  return ['date', 'title', 'data', 'threat'].every(k =>
    value[k] === undefined || typeof value[k] === 'string') &&
    (value.cities === undefined ||
      (Array.isArray(value.cities) && value.cities.every(city => typeof city === 'string')));
}

function isAlertsUaState(value: unknown): value is AlertsUaState {
  if (!isRecord(value) || typeof value.alert !== 'boolean') return false;
  return ['name', 'name_en', 'changed'].every(k =>
    value[k] === undefined || typeof value[k] === 'string') &&
    (value.id === undefined || typeof value.id === 'number');
}

interface AlertsUaState {
  id?: number;
  name?: string;
  name_en?: string;
  alert: boolean;
  changed?: string;
}

interface TzevaAdomAlert {
  date?: string;
  title?: string;
  data?: string;
  threat?: string;
  cities?: string[];
}

function categorizeAlert(threat: string): string {
  const t = threat.toLowerCase();
  if (t.includes('missile') || t.includes('טיל') || t.includes('ballistic')) return 'MISSILE';
  if (t.includes('rocket') || t.includes('רקט')) return 'ROCKET';
  if (t.includes('drone') || t.includes('uav') || t.includes('כטב') || t.includes('hostile aircraft')) return 'DRONE';
  if (t.includes('mortar')) return 'MORTAR';
  if (t.includes('infiltration') || t.includes('חדיר')) return 'INFILTRATION';
  if (t.includes('earthquake') || t.includes('רעידת')) return 'EARTHQUAKE';
  if (t.includes('tsunami')) return 'TSUNAMI';
  if (t.includes('chemical') || t.includes('hazmat')) return 'HAZMAT';
  return 'ALERT';
}
