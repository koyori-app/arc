import { beforeEach, describe, expect, it, vi } from 'vitest';
import { loadLegendSpec } from './legend';

const calls: string[] = [];
const spec = {
  tiers: [{ key: 'k', label: 'L', color: '#000', min_pct: 0, max_pct: 100 }],
  progress_line: { label: 'P', color: '#111', stroke_width: 3, dash: '1,2' },
};

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(async () => { calls.push('init'); }),
  legend_spec_json: () => { calls.push('legend_spec_json'); return JSON.stringify(spec); },
}));

describe('loadLegendSpec', () => {
  beforeEach(() => { calls.length = 0; });

  it('returns what the loaded Wasm says, not a value of its own', async () => {
    expect(await loadLegendSpec()).toEqual(spec);
  });

  it('initializes the Wasm before it asks, so it can run before any chart has mounted', async () => {
    await loadLegendSpec();
    expect(calls).toEqual(['init', 'legend_spec_json']);
  });
});
