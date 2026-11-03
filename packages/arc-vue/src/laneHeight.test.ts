import { describe, expect, it } from 'vitest';
import { estimateLaneChartHeight, svgRootHeight } from './laneHeight';
import { RUST_LAYOUT } from './wasmContract';

describe('estimateLaneChartHeight', () => {
  it('is one row a lane plus the header, the legend and the bottom padding', () => {
    expect(estimateLaneChartHeight(3)).toBe(3 * 40 + 30 + 40 + 10);
    expect(estimateLaneChartHeight(3, 'hidden')).toBe(3 * 40 + 30 + 10);
  });

  it('takes the legend off by exactly the legend height', () => {
    expect(estimateLaneChartHeight(2) - estimateLaneChartHeight(2, 'hidden')).toBe(RUST_LAYOUT.LEGEND_H);
  });
});

describe('svgRootHeight', () => {
  it('reads the height of the root element', () => {
    expect(svgRootHeight('<svg xmlns="http://www.w3.org/2000/svg" width="470" height="320" viewBox="0 0 470 320">x</svg>')).toBe(320);
  });

  it('does not read the height of an element inside', () => {
    expect(svgRootHeight('<svg width="1"><rect height="9"/></svg>')).toBeNull();
  });

  it('is null for something that is not a drawing', () => {
    expect(svgRootHeight('')).toBeNull();
    expect(svgRootHeight('chart')).toBeNull();
  });
});
