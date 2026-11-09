/**
 * @vitest-environment happy-dom
 */
import { describe, expect, it } from 'vitest';
import { buildOptionsJson } from './options';
import { replayCommands, resolveCssColor, type CommandBuffer } from './replayCommands';

describe('palette overrides', () => {
  it('are written to the options only when given', () => {
    expect(JSON.parse(buildOptionsJson({ palette: { HeaderBg: 'var(--x)' } })!)).toEqual({
      palette: { HeaderBg: 'var(--x)' },
    });
    expect(buildOptionsJson({ palette: null })).toBeUndefined();
  });

  it('read `var(--name)` off the element for the canvas, and keep any other colour', () => {
    const el = document.createElement('canvas');
    el.style.setProperty('--koyori-color-surface', ' #faf8f5 ');
    document.body.append(el);
    expect(resolveCssColor('var(--koyori-color-surface)', el)).toBe('#faf8f5');
    expect(resolveCssColor('var(--not-set)', el)).toBe('#000000');
    expect(resolveCssColor('#123456', el)).toBe('#123456');
    expect(resolveCssColor('rgb(1, 2, 3)', undefined)).toBe('rgb(1, 2, 3)');
    el.remove();
  });

  it('draw a group row with the header colour until GroupRow is set', () => {
    const fills: string[] = [];
    // Every method a no-op; only `fillStyle` and `canvas` matter here.
    const canvas = document.createElement('canvas');
    const ctx = new Proxy(
      {},
      {
        get: (_t, key) => (key === 'canvas' ? canvas : () => {}),
        set: (_t, key, value) => {
          if (key === 'fillStyle') fills.push(value as string);
          return true;
        },
      },
    ) as unknown as CanvasRenderingContext2D;
    const buffer = (palette: [string, string][]): CommandBuffer => ({
      viewport_width: 10,
      viewport_height: 10,
      ops: [{ FillRect: { x: 0, y: 0, w: 1, h: 1, color_id: 13, radius: 0 } }],
      palette: { colors: palette as CommandBuffer['palette']['colors'] },
    });
    replayCommands(ctx, buffer([['HeaderBg', '#f3f4f6']]));
    replayCommands(ctx, buffer([['HeaderBg', '#f3f4f6'], ['GroupRow', '#eeeeee']]));
    expect(fills).toEqual(['#f3f4f6', '#eeeeee']);
  });

  it('draw a weekend band with the header colour until Weekend is set', () => {
    const fills: string[] = [];
    const canvas = document.createElement('canvas');
    const ctx = new Proxy(
      {},
      {
        get: (_t, key) => (key === 'canvas' ? canvas : () => {}),
        set: (_t, key, value) => {
          if (key === 'fillStyle') fills.push(value as string);
          return true;
        },
      },
    ) as unknown as CanvasRenderingContext2D;
    const buffer: CommandBuffer = {
      viewport_width: 10,
      viewport_height: 10,
      ops: [{ FillRect: { x: 0, y: 0, w: 1, h: 1, color_id: 14, radius: 0 } }],
      palette: { colors: [['HeaderBg', '#f3f4f6']] },
    };
    replayCommands(ctx, buffer);
    expect(fills).toEqual(['#f3f4f6']);
  });

  it('draw the marks with the colours they fall back to until they are set', () => {
    const fills: string[] = [];
    const strokes: string[] = [];
    const canvas = document.createElement('canvas');
    const ctx = new Proxy(
      {},
      {
        get: (_t, key) => (key === 'canvas' ? canvas : () => {}),
        set: (_t, key, value) => {
          if (key === 'fillStyle') fills.push(value as string);
          if (key === 'strokeStyle') strokes.push(value as string);
          return true;
        },
      },
    ) as unknown as CanvasRenderingContext2D;
    const buffer = (palette: [string, string][]): CommandBuffer => ({
      viewport_width: 10,
      viewport_height: 10,
      ops: [
        // WarnBg、WarnStroke、HardDeadline、Note
        { FillRect: { x: 0, y: 0, w: 1, h: 1, color_id: 18, radius: 0 } },
        // jsdom に Path2D が無いので、縁の色は折れ線で確かめる
        { StrokePolyline: { points: [[0, 0], [1, 0]], color_id: 19, width: 1, dash: null } },
        { FillPolygon: { points: [[0, 0], [1, 1], [0, 1]], color_id: 20 } },
        {
          DrawText: { x: 0, y: 0, text: 'n', color_id: 21, anchor: 0, size: 11, weight: 400 },
        },
      ],
      palette: { colors: palette as CommandBuffer['palette']['colors'] },
    });
    const standard: [string, string][] = [
      ['BarBg', '#d1d5db'],
      ['Progress', '#dc2626'],
      ['GridLabel', '#6b7280'],
    ];
    replayCommands(ctx, buffer(standard));
    expect(fills).toEqual(['#d1d5db', '#dc2626', '#6b7280']);
    expect(strokes).toContain('#dc2626');
    fills.length = 0;
    replayCommands(
      ctx,
      buffer([...standard, ['WarnBg', '#fef3c7'], ['HardDeadline', '#111111'], ['Note', '#222222']]),
    );
    expect(fills).toEqual(['#fef3c7', '#111111', '#222222']);
  });
});
