/**
 * @vitest-environment happy-dom
 *
 * The colour a task row gives its bar, as the canvas path replays it
 * (docs/design/row-bar-color.md, 4.5): tags from 256 up name the row colours
 * the chart added to the palette, `FillRectAlpha` paints the faint part of a
 * shaded bar, and `DrawHaloText` strokes a band round a label before filling it.
 */
import { describe, expect, it } from 'vitest';
import { buildOptionsJson } from './options';
import { replayCommands, type CommandBuffer, type DrawOp } from './replayCommands';

interface Call {
  op: string;
  args?: unknown[];
  fillStyle?: unknown;
  strokeStyle?: unknown;
  globalAlpha?: number;
  lineWidth?: number;
  lineJoin?: string;
}

/** A context that records what each draw call was made with. */
function recordingContext(canvas: HTMLCanvasElement = document.createElement('canvas')) {
  const calls: Call[] = [];
  const state: Record<string, unknown> = { globalAlpha: 1 };
  const ctx = new Proxy(state, {
    get: (_t, key) => {
      if (key === 'canvas') return canvas;
      if (typeof key === 'string' && key in state) return state[key];
      return (...args: unknown[]) =>
        calls.push({
          op: String(key),
          args,
          fillStyle: state.fillStyle,
          strokeStyle: state.strokeStyle,
          globalAlpha: state.globalAlpha as number,
          lineWidth: state.lineWidth as number,
          lineJoin: state.lineJoin as string,
        });
    },
    set: (_t, key, value) => {
      state[key as string] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return { ctx, calls, state };
}

function buffer(ops: DrawOp[], colors: CommandBuffer['palette']['colors']): CommandBuffer {
  return { viewport_width: 100, viewport_height: 40, ops, palette: { colors } };
}

const standard: CommandBuffer['palette']['colors'] = [
  ['BarBg', '#d1d5db'],
  ['GridLabel', '#6b7280'],
  ['ProgressTextOnFg', '#ffffff'],
  ['ProgressTextOnBg', '#374151'],
];

describe('the row colours on the canvas', () => {
  it('reads a tag from 256 up as the nth row colour', () => {
    const { ctx, calls } = recordingContext();
    replayCommands(
      ctx,
      buffer(
        [
          { FillRect: { x: 0, y: 0, w: 10, h: 10, color_id: 257, radius: 0 } },
          { FillRect: { x: 0, y: 0, w: 10, h: 10, color_id: 256, radius: 0 } },
          { FillRect: { x: 0, y: 0, w: 10, h: 10, color_id: 258, radius: 0 } },
        ],
        [...standard, [{ Row: 0 }, '#ffff00'], [{ Row: 1 }, 'oklch(0.5 0.1 200)']],
      ),
    );
    expect(calls.filter((c) => c.op === 'fillRect').map((c) => c.fillStyle)).toEqual([
      'oklch(0.5 0.1 200)',
      '#ffff00',
      // A row colour the palette does not have draws black, as any missing colour.
      '#000000',
    ]);
  });

  it('reads a row colour written var(--name) off the canvas', () => {
    const canvas = document.createElement('canvas');
    canvas.style.setProperty('--status', '#1e3a8a');
    document.body.appendChild(canvas);
    const { ctx, calls } = recordingContext(canvas);
    replayCommands(
      ctx,
      buffer(
        [{ FillRect: { x: 0, y: 0, w: 10, h: 10, color_id: 256, radius: 0 } }],
        [...standard, [{ Row: 0 }, 'var(--status)']],
      ),
    );
    canvas.remove();
    expect(calls.find((c) => c.op === 'fillRect')?.fillStyle).toBe('#1e3a8a');
  });

  it('paints FillRectAlpha at its opacity and puts the opacity back', () => {
    const { ctx, calls, state } = recordingContext();
    replayCommands(
      ctx,
      buffer(
        [
          {
            FillRectAlpha: { x: 0, y: 0, w: 10, h: 10, color_id: 256, radius: 0, alpha: 0.35 },
          },
          { FillRect: { x: 0, y: 0, w: 5, h: 10, color_id: 256, radius: 0 } },
        ],
        [...standard, [{ Row: 0 }, '#ffff00']],
      ),
    );
    const fills = calls.filter((c) => c.op === 'fillRect');
    expect(fills.map((c) => [c.fillStyle, c.globalAlpha])).toEqual([
      ['#ffff00', 0.35],
      ['#ffff00', 1],
    ]);
    expect(state.globalAlpha).toBe(1);
  });

  it('counts FillRectAlpha in the task hit region', () => {
    const { ctx } = recordingContext();
    const { hitRegions } = replayCommands(
      ctx,
      buffer(
        [
          { GroupStart: { task_id: 'a' } },
          {
            FillRectAlpha: { x: 4, y: 6, w: 20, h: 10, color_id: 256, radius: 0, alpha: 0.35 },
          },
          'GroupEnd',
        ],
        [...standard, [{ Row: 0 }, '#ffff00']],
      ),
    );
    expect(hitRegions).toEqual([{ taskId: 'a', x: 4, y: 6, w: 20, h: 10 }]);
  });

  it('strokes the halo round a label before it fills the label', () => {
    const { ctx, calls } = recordingContext();
    replayCommands(
      ctx,
      buffer(
        [
          {
            DrawHaloText: {
              x: 50,
              y: 20,
              text: '50%',
              color_id: 27,
              anchor: 1,
              size: 11,
              weight: 600,
              halo_color_id: 28,
              halo_width: 3,
            },
          },
        ],
        standard,
      ),
    );
    const text = calls.filter((c) => c.op === 'strokeText' || c.op === 'fillText');
    expect(text.map((c) => c.op)).toEqual(['strokeText', 'fillText']);
    // `ColoredBarHalo` and `ColoredBarText` fall back to the two progress text colours.
    expect(text[0]).toMatchObject({ strokeStyle: '#ffffff', lineWidth: 3, lineJoin: 'round' });
    expect(text[1]).toMatchObject({ fillStyle: '#374151' });
    expect(text[1].args).toEqual(['50%', 50, 20]);
  });

  it('takes the halo colours a caller sets', () => {
    const { ctx, calls } = recordingContext();
    replayCommands(
      ctx,
      buffer(
        [
          {
            DrawHaloText: {
              x: 0,
              y: 0,
              text: '1%',
              color_id: 27,
              anchor: 0,
              size: 11,
              weight: 600,
              halo_color_id: 28,
              halo_width: 3,
            },
          },
        ],
        [...standard, ['ColoredBarText', '#f9fafb'], ['ColoredBarHalo', '#111827']],
      ),
    );
    expect(calls.find((c) => c.op === 'strokeText')?.strokeStyle).toBe('#111827');
    expect(calls.find((c) => c.op === 'fillText')?.fillStyle).toBe('#f9fafb');
  });
});

describe('the row colour choices', () => {
  it('are written as options only when given', () => {
    expect(buildOptionsJson({ rowColorProgress: null, rowColorPercent: null })).toBeUndefined();
    expect(
      JSON.parse(buildOptionsJson({ rowColorProgress: 'line', rowColorPercent: 'hidden' })!),
    ).toEqual({ row_color_progress: 'line', row_color_percent: 'hidden' });
  });
});
