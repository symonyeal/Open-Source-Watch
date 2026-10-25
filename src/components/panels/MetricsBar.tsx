'use client';

import { useConflictFeed, formatPrice } from '@/lib/hooks';
import type { OilData } from '@/lib/types/api';

export default function MetricsBar() {
  const { data: oilData } = useConflictFeed<OilData[]>('/api/oil', 600000);

  const wti = oilData?.find(o => o.type === 'crude_wti');
  const brent = oilData?.find(o => o.type === 'crude_brent');
  const natGas = oilData?.find(o => o.type === 'natural_gas');

  const metrics = [
    {
      label: 'WTI FUTURES',
      value: !wti ? '---' : wti.error ? 'N/A' : `${formatPrice(wti.price)} USD/bbl`,
      change: wti?.error ? 0 : wti?.changePercent || 0,
      error: wti?.error,
    },
    {
      label: 'BRENT FUTURES',
      value: !brent ? '---' : brent.error ? 'N/A' : `${formatPrice(brent.price)} USD/bbl`,
      change: brent?.error ? 0 : brent?.changePercent || 0,
      error: brent?.error,
    },
    {
      label: 'NAT GAS FUTURES',
      value: !natGas ? '---' : natGas.error ? 'N/A' : `${formatPrice(natGas.price)} USD/MMBtu`,
      change: natGas?.error ? 0 : natGas?.changePercent || 0,
      error: natGas?.error,
    },
  ];

  return (
    <div className="flex items-center gap-1 overflow-x-auto py-1">
      {metrics.map((m, i) => (
        <div
          key={i}
          className="flex items-center gap-3 px-4 py-2 border-r border-[var(--border-color)] last:border-r-0 whitespace-nowrap"
        >
          <span className="text-[10px] text-[var(--text-secondary)] tracking-wider">
            {m.label}
          </span>
          <span
            className={`text-sm font-bold ${
              m.error
                ? 'text-[var(--text-secondary)]'
                : m.change > 0
                ? 'value-up'
                : m.change < 0
                ? 'value-down'
                : 'text-[var(--text-primary)]'
            }`}
          >
            {m.value}
          </span>
          {!m.error && m.change !== 0 && (
            <span
              className={`text-[10px] ${
                m.change > 0 ? 'value-up' : 'value-down'
              }`}
            >
              {m.change > 0 ? '+' : ''}
              {m.change.toFixed(2)}%
            </span>
          )}
        </div>
      ))}
    </div>
  );
}
