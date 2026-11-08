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
});
