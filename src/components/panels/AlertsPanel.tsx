'use client';

import { useEffect, useRef, useState } from 'react';
import { useConflictFeed } from '@/lib/hooks';
import { useConflict } from '@/lib/conflicts/context';
import { playAlertSound } from '@/lib/generateAlert';
import type { AlertData, DroneData } from '@/lib/types/api';

const TYPE_ICONS: Record<string, string> = {
  MISSILE: '🚀',
  ROCKET: '🎯',
  DRONE: '✈',
  MORTAR: '💣',
  INFILTRATION: '⚠',
  ALERT: '🔴',
};

export default function AlertsPanel() {
  const { config } = useConflict();
  const alertSystemName = config.client.alertSystemName;
  const { data, loading, error } = useConflictFeed<AlertData>('/api/alerts', 15000);
  const { data: droneData } = useConflictFeed<DroneData>('/api/drones', 20000);
  const drones = droneData?.drones || [];
  const prevStatus = useRef<string>('CLEAR');
  const [soundEnabled, setSoundEnabled] = useState(true);
  const [hasInteracted, setHasInteracted] = useState(false);

  // Enable audio context after first user interaction (browser requirement)
  useEffect(() => {
    const handleInteraction = () => {
      setHasInteracted(true);
      window.removeEventListener('click', handleInteraction);
      window.removeEventListener('keydown', handleInteraction);
    };
    window.addEventListener('click', handleInteraction);
    window.addEventListener('keydown', handleInteraction);
    return () => {
      window.removeEventListener('click', handleInteraction);
      window.removeEventListener('keydown', handleInteraction);
    };
  }, []);

  // Play sound when status changes to ACTIVE
  useEffect(() => {
    if (!data) return;

    if (data.status === 'ACTIVE' && prevStatus.current !== 'ACTIVE' && soundEnabled && hasInteracted) {
      // Play urgent sound for new alerts
      playAlertSound('urgent');

      // Also send browser notification if permitted
      if (typeof Notification !== 'undefined' && Notification.permission === 'granted') {
        new Notification('IRONSIGHT ALERT', {
          body: `${data.activeCount} active alert(s) - ${data.alerts[0]?.type}: ${data.alerts[0]?.threat}`,
          icon: '/favicon.ico',
          tag: 'ironsight-alert',
        });
      }
    }

    prevStatus.current = data.status;
  }, [data, soundEnabled, hasInteracted]);

  // Request notification permission
  useEffect(() => {
    if (typeof Notification !== 'undefined' && Notification.permission === 'default') {
      Notification.requestPermission();
    }
  }, []);

  const isActive = !error && data?.status === 'ACTIVE';
  const isClear = !error && data?.status === 'CLEAR';
  const isUnavailable = data?.status === 'UNAVAILABLE';
  const isUnknown = !!error || data?.status === 'UNKNOWN' || (!loading && !data);
  const statusUnknown = isUnknown || isUnavailable || loading;
  const hasThreat = isActive || (!statusUnknown && drones.length > 0);
  const stateColor = hasThreat ? 'var(--red)' : isClear ? 'var(--green)' : 'var(--text-secondary)';
  const stateLabel = isActive
    ? `${data.activeCount} ALERT${drones.length ? ` · ${drones.length} TRK` : ''}`
    : isClear
      ? drones.length ? `${drones.length} TRK` : 'ALL CLEAR'
      : isUnavailable ? 'UNAVAILABLE'
        : loading ? 'CHECKING' : `STATUS UNKNOWN${drones.length ? ` · ${drones.length} TRK` : ''}`;

  return (
    <div
      className="panel h-full flex flex-col"
      style={hasThreat ? { borderColor: 'var(--red)', boxShadow: '0 0 20px rgba(255, 51, 102, 0.3)' } : {}}
    >
      <div className="panel-header">
        <span
          className="status-dot"
          style={{
            background: stateColor,
            animation: hasThreat ? 'pulse-dot 0.5s ease-in-out infinite' : undefined,
          }}
        />
        {config.client.alertStatusTitle}
        <div className="ml-auto flex items-center gap-2">
          {/* Sound toggle */}
          <button
            onClick={() => {
              setSoundEnabled(!soundEnabled);
              if (!soundEnabled && hasInteracted) {
                playAlertSound('ping'); // Preview sound when enabling
              }
            }}
            className="text-[9px] px-1.5 py-0.5 rounded border transition-colors"
            style={{
              color: soundEnabled ? 'var(--cyan)' : 'var(--text-secondary)',
              borderColor: soundEnabled ? 'var(--cyan)' : 'var(--border-color)',
              background: soundEnabled ? 'rgba(0,212,255,0.1)' : 'transparent',
            }}
            title={soundEnabled ? 'Sound alerts ON' : 'Sound alerts OFF'}
          >
            {soundEnabled ? '🔔' : '🔕'}
          </button>
          <span className="text-[9px] font-normal normal-case tracking-normal"
            style={{ color: stateColor }}
          >
            {stateLabel}
          </span>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {drones.length > 0 && (
          <div className="border-b border-[var(--border-color)]">
            <div className="px-3 py-1 text-[9px] tracking-widest text-[var(--text-secondary)] bg-[var(--bg-panel-header)]">
              LIVE TRACKS // {drones.length} INBOUND
            </div>
            {drones.slice(0, 12).map(d => (
              <div key={d.id} className="data-row flex items-center gap-2">
                <span className="text-sm" style={{ color: d.color, transform: `rotate(${d.heading}deg)` }}>▲</span>
                <div className="flex-1 min-w-0">
                  <div className="text-[10px] font-bold" style={{ color: d.color }}>
                    {d.label}{d.count > 1 ? ` ×${d.count}` : ''}
                    {d.place ? <span className="text-[var(--text-primary)] font-normal"> → {d.place}</span> : null}
                  </div>
                  <div className="text-[8px] text-[var(--text-secondary)]">
                    hdg {d.heading}° · {new Date(d.time).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
        {loading ? (
          <div className="p-3">
            <div className="loading-shimmer h-20 rounded" />
          </div>
        ) : isUnknown ? (
          <div className="p-3 text-[var(--text-secondary)]">
            <div className="text-xs font-bold mb-1">ALERT STATUS UNKNOWN</div>
            <div className="text-[9px]">
              {error ? 'The latest provider check failed.' : 'The provider response could not be confirmed.'}
              {drones.length ? ` ${drones.length} threat track(s) are shown above.` : ''}
            </div>
            {!!data?.alerts.length && (
              <AlertRows alerts={data.alerts} label="Last observed" />
            )}
          </div>
        ) : isUnavailable ? (
          <div className="p-3 text-[9px] text-[var(--text-secondary)]">
            <div className="text-xs font-bold mb-1">ALERT FEED UNAVAILABLE</div>
            No alert provider is configured for {alertSystemName}.
            {drones.length ? ` ${drones.length} threat track(s) are shown above.` : ''}
          </div>
        ) : isActive ? (
          <>
            {/* Active alert banner */}
            <div className="px-3 py-2 bg-red-900/30 border-b border-red-800/50 alert-flash">
              <div className="flex items-center gap-2">
                <span className="text-lg">🚨</span>
                <div>
                  <div className="text-xs font-bold text-[var(--red)]">
                    INCOMING THREAT DETECTED
                  </div>
                  <div className="text-[9px] text-[var(--text-secondary)]">
                    {alertSystemName} sirens activated
                  </div>
                </div>
              </div>
            </div>

            <AlertRows alerts={data?.alerts || []} />
          </>
        ) : drones.length > 0 ? (
          // Drones inbound but no oblast siren active — the LIVE TRACKS section above covers it
          <div className="px-3 py-2 text-[9px] text-[var(--text-secondary)]">
            {isClear
              ? `No oblast air-raid sirens active · tracking ${drones.length} inbound threat${drones.length > 1 ? 's' : ''} above`
              : `Alert status is ${isUnavailable ? 'unavailable' : 'unknown'} · tracking ${drones.length} inbound threat${drones.length > 1 ? 's' : ''} above`}
          </div>
        ) : isClear ? (
          <div className="flex flex-col items-center justify-center h-full p-4">
            <div className="relative w-16 h-16 mb-3">
              <svg viewBox="0 0 24 24" fill="none" className="w-full h-full" stroke="var(--green)" strokeWidth="1.5">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" fill="rgba(0,255,136,0.1)" />
                <path d="M9 12l2 2 4-4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
            <div className="text-sm font-bold text-[var(--green)] mb-1">ALL CLEAR</div>
            <div className="text-[9px] text-[var(--text-secondary)] text-center">
              No active alerts from {alertSystemName}
            </div>
            {!!data.alerts.length && <AlertRows alerts={data.alerts} label="Recently ended" />}
            <div className="text-[8px] text-[var(--text-secondary)] mt-2">
              Polling every 15s • Last: {data?.lastChecked ? new Date(data.lastChecked).toLocaleTimeString() : '...'}
            </div>
            <div className="text-[8px] mt-1" style={{ color: soundEnabled ? 'var(--cyan)' : 'var(--text-secondary)' }}>
              Sound: {soundEnabled ? 'ON' : 'OFF'} {!hasInteracted && soundEnabled ? '(click anywhere to enable)' : ''}
            </div>
          </div>
        ) : (
          <div className="p-3 text-[9px] text-[var(--text-secondary)]">Alert status unknown.</div>
        )}
      </div>
    </div>
  );
}

function AlertRows({ alerts, label }: { alerts: AlertData['alerts']; label?: string }) {
  return <div className="mt-2">
    {label && <div className="text-[8px] tracking-wider mb-1">{label.toUpperCase()}</div>}
    {alerts.map((alert, i) => (
      <div key={i} className="data-row">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-sm">{TYPE_ICONS[alert.type] || '🔴'}</span>
          <span className={`text-[10px] font-bold ${alert.active ? 'text-[var(--red)]' : 'text-[var(--text-secondary)]'}`}>
            {alert.type}{!alert.active ? ' · ENDED' : ''}
          </span>
          <span className="text-[9px] text-[var(--text-secondary)]">
            {new Date(alert.time).toLocaleTimeString()}
          </span>
        </div>
        <div className="text-[11px] text-[var(--text-primary)]">{alert.threat}</div>
        <div className="text-[9px] text-[var(--text-secondary)] mt-0.5">{alert.locations.join(', ')}</div>
      </div>
    ))}
  </div>;
}
