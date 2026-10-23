import { describe, expect, it } from 'vitest';
import { backingScale, backingSize, cssPointFromClient } from './canvasScale';
import { RUST_CANVAS_LIMITS } from './wasmContract';

const { MAX_CANVAS_SIDE_PX, MAX_CANVAS_AREA_PX } = RUST_CANVAS_LIMITS;

describe('backingScale', () => {
  it('上限に余裕がある時は devicePixelRatio をそのまま使う', () => {
    expect(backingScale(800, 600, 1)).toBe(1);
    expect(backingScale(800, 600, 1.5)).toBe(1.5);
    expect(backingScale(800, 600, 2)).toBe(2);
  });

  it('一辺の上限で頭打ちにする', () => {
    // 高さ 12,000px を 2 倍にすると 24,000px になり、一辺の上限を越える。
    const scale = backingScale(100, 12_000, 2);
    expect(scale).toBeCloseTo(MAX_CANVAS_SIDE_PX / 12_000);
    expect(backingSize(100, 12_000, 2).height).toBeLessThanOrEqual(MAX_CANVAS_SIDE_PX);
  });

  it('面積の上限で頭打ちにする', () => {
    // 評価で使った 300 行の canvas（2210×12080）は、1.5 倍で面積の上限を越える。
    const size = backingSize(2210, 12_080, 1.5);
    expect(size.width * size.height).toBeLessThanOrEqual(MAX_CANVAS_AREA_PX);
    expect(backingScale(2210, 12_080, 1.5)).toBeGreaterThan(1);
    expect(backingScale(2210, 12_080, 1.5)).toBeLessThan(1.5);
  });

  it('寸法や倍率が使えない値の時は 1 にする', () => {
    expect(backingScale(0, 600, 2)).toBe(1);
    expect(backingScale(800, 0, 2)).toBe(1);
    expect(backingScale(800, 600, 0)).toBe(1);
    expect(backingScale(800, 600, Number.NaN)).toBe(1);
    expect(backingScale(800, 600, Number.POSITIVE_INFINITY)).toBe(1);
  });

  it('縮小表示で devicePixelRatio が 1 を割る時は、そのまま小さい裏打ちにする', () => {
    expect(backingScale(800, 600, 0.75)).toBe(0.75);
  });
});

describe('backingSize', () => {
  it('端数を切り捨てる', () => {
    expect(backingSize(101, 51, 1.5)).toEqual({ width: 151, height: 76 });
  });
});

describe('cssPointFromClient', () => {
  const rect = { left: 10, top: 20, width: 400, height: 300 };

  it('裏打ちの倍率に関わらず CSS の座標を返す', () => {
    expect(cssPointFromClient(110, 70, rect, 400, 300)).toEqual({ x: 100, y: 50 });
  });

  it('外から CSS で縮められた時は、表示の寸法との比で戻す', () => {
    const shrunk = { left: 0, top: 0, width: 200, height: 150 };
    expect(cssPointFromClient(50, 30, shrunk, 400, 300)).toEqual({ x: 100, y: 60 });
  });

  it('表示の寸法が 0 の時は点を返さない', () => {
    expect(cssPointFromClient(0, 0, { left: 0, top: 0, width: 0, height: 0 }, 400, 300))
      .toBeNull();
  });
});
