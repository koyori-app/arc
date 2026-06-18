import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import { findTaskAtPoint, parseCommandBuffer, replayCommands } from './replayCommands.ts';

const __dirname = dirname(fileURLToPath(import.meta.url));
const goldenPath = join(
  __dirname,
  '../../../crates/koyori-arc-core/tests/fixtures/canvas_golden/two_tasks.json',
);
const goldenJson = readFileSync(goldenPath, 'utf8');

beforeAll(() => {
  class MockPath2D {
    d: string;
    constructor(d: string) {
      this.d = d;
    }
  }
  globalThis.Path2D = MockPath2D as unknown as typeof Path2D;
});

function createMockCtx() {
  const calls: string[] = [];
  const ctx = {
    calls,
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 0,
    font: '',
    textAlign: 'start' as CanvasTextAlign,
    textBaseline: 'alphabetic' as CanvasTextBaseline,
    lineDash: [] as number[],
    clearRect(_x: number, _y: number, _w: number, _h: number) {
      calls.push('clearRect');
    },
    fillRect(x: number, y: number, w: number, h: number) {
      calls.push(`fillRect:${x},${y},${w},${h}`);
    },
    roundRect(x: number, y: number, w: number, h: number, r: number) {
      calls.push(`roundRect:${x},${y},${w},${h},${r}`);
    },
    beginPath() {
      calls.push('beginPath');
    },
    moveTo(x: number, y: number) {
      calls.push(`moveTo:${x},${y}`);
    },
    lineTo(x: number, y: number) {
      calls.push(`lineTo:${x},${y}`);
    },
    closePath() {
      calls.push('closePath');
    },
    fill() {
      calls.push('fill');
    },
    stroke(path?: Path2D) {
      if (path && 'd' in path) {
        calls.push(`strokePath:${(path as { d: string }).d.slice(0, 20)}`);
      } else {
        calls.push('stroke');
      }
    },
    fillText(text: string, x: number, y: number) {
      calls.push(`fillText:${text}@${x},${y}`);
    },
    setLineDash(dash: number[]) {
      this.lineDash = dash;
      calls.push(`setLineDash:${dash.join(',')}`);
    },
  };
  return ctx as unknown as CanvasRenderingContext2D & { calls: string[]; lineDash: number[] };
}

describe('replayCommands', () => {
  it('parses golden CommandBuffer JSON', () => {
    const buffer = parseCommandBuffer(goldenJson);
    expect(buffer.viewport_width).toBe(350);
    expect(buffer.ops.length).toBeGreaterThan(10);
    expect(buffer.palette.colors.length).toBeGreaterThan(0);
  });

  it('replays golden buffer without throwing and records draw calls', () => {
    const buffer = parseCommandBuffer(goldenJson);
    const ctx = createMockCtx();
    const result = replayCommands(ctx, buffer);
    expect(ctx.calls[0]).toBe('clearRect');
    expect(ctx.calls.some((c) => c.startsWith('fillRect:') || c.startsWith('roundRect:'))).toBe(true);
    expect(ctx.calls.some((c) => c.startsWith('fillText:'))).toBe(true);
    expect(ctx.calls.filter((c) => c === 'stroke').length).toBeGreaterThan(0);
    expect(result.hitRegions.some((r) => r.taskId === 'task-1')).toBe(true);
    expect(result.hitRegions.some((r) => r.taskId === 'task-2')).toBe(true);
  });

  it('replays progress line polyline with dash pattern', () => {
    const buffer = parseCommandBuffer(goldenJson);
    const ctx = createMockCtx();
    replayCommands(ctx, buffer);
    expect(ctx.lineDash).toEqual([6, 3]);
    const progressOp = buffer.ops.find(
      (op) => typeof op === 'object' && 'StrokePolyline' in op && op.StrokePolyline.color_id === 6,
    );
    expect(progressOp).toBeDefined();
  });

  it('findTaskAtPoint returns task inside hit region', () => {
    const buffer = parseCommandBuffer(goldenJson);
    const ctx = createMockCtx();
    const { hitRegions } = replayCommands(ctx, buffer);
    const region = hitRegions.find((r) => r.taskId === 'task-1');
    expect(region).toBeDefined();
    const cx = region!.x + region!.w / 2;
    const cy = region!.y + region!.h / 2;
    expect(findTaskAtPoint(hitRegions, cx, cy)).toBe('task-1');
  });
});
