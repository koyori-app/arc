/** Mirrors koyori-arc-core `CommandBuffer` JSON (serde externally-tagged `DrawOp`). */

export type ColorIdName =
  | 'BarBg'
  | 'TierLow'
  | 'TierMid'
  | 'TierHigh'
  | 'TierDone'
  | 'Dep'
  | 'Progress'
  | 'Grid'
  | 'Today'
  | 'HeaderBg'
  | 'GridLabel'
  | 'ProgressTextOnFg'
  | 'ProgressTextOnBg';

export interface CommandBuffer {
  viewport_width: number;
  viewport_height: number;
  ops: DrawOp[];
  palette: { colors: [ColorIdName, string][] };
  error?: string;
}

export type DrawOp =
  | { FillRect: { x: number; y: number; w: number; h: number; color_id: number; radius: number } }
  | { StrokePath: { d: string; color_id: number; width: number } }
  | {
      StrokePolyline: {
        points: [number, number][];
        color_id: number;
        width: number;
        dash: string | null;
      };
    }
  | { FillPolygon: { points: [number, number][]; color_id: number } }
  | {
      DrawText: {
        x: number;
        y: number;
        text: string;
        color_id: number;
        anchor: number;
        size: number;
        weight: number;
      };
    }
  | { GroupStart: { task_id: string | null } }
  | 'GroupEnd';

export interface TaskHitRegion {
  taskId: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ReplayResult {
  hitRegions: TaskHitRegion[];
}

/** u8 tags from `canvas.rs::color_id_tag` — must stay in sync with Rust. */
const COLOR_ID_NAMES: ColorIdName[] = [
  'BarBg',
  'TierLow',
  'TierMid',
  'TierHigh',
  'TierDone',
  'Dep',
  'Progress',
  'Grid',
  'Today',
  'HeaderBg',
  'GridLabel',
  'ProgressTextOnFg',
  'ProgressTextOnBg',
];

function resolveColor(buffer: CommandBuffer, colorId: number): string {
  if (colorId === 255) return '#374151';
  const name = COLOR_ID_NAMES[colorId];
  if (!name) return '#000000';
  const entry = buffer.palette.colors.find(([id]) => id === name);
  return entry?.[1] ?? '#000000';
}

function unionRect(
  r: { x: number; y: number; w: number; h: number },
  x: number,
  y: number,
  w: number,
  h: number,
) {
  if (w <= 0 || h <= 0) return r;
  const x2 = x + w;
  const y2 = y + h;
  if (r.w === 0 && r.h === 0) return { x, y, w, h };
  const nx = Math.min(r.x, x);
  const ny = Math.min(r.y, y);
  const nx2 = Math.max(r.x + r.w, x2);
  const ny2 = Math.max(r.y + r.h, y2);
  return { x: nx, y: ny, w: nx2 - nx, h: ny2 - ny };
}

function fillRoundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  radius: number,
) {
  if (radius > 0 && typeof ctx.roundRect === 'function') {
    ctx.beginPath();
    ctx.roundRect(x, y, w, h, radius);
    ctx.fill();
    return;
  }
  ctx.fillRect(x, y, w, h);
}

function applyStrokeStyle(
  ctx: CanvasRenderingContext2D,
  buffer: CommandBuffer,
  colorId: number,
  width: number,
  dash: string | null,
) {
  ctx.strokeStyle = resolveColor(buffer, colorId);
  ctx.lineWidth = width;
  if (dash) {
    const parts = dash.split(',').map((s) => Number.parseFloat(s.trim()));
    ctx.setLineDash(parts.every(Number.isFinite) ? parts : []);
  } else {
    ctx.setLineDash([]);
  }
}

/**
 * Replay a wasm-produced `CommandBuffer` onto a Canvas2D context.
 * Single JS loop — no per-op wasm round-trips.
 */
export function replayCommands(
  ctx: CanvasRenderingContext2D,
  buffer: CommandBuffer,
): ReplayResult {
  const hitRegions: TaskHitRegion[] = [];
  let currentTaskId: string | null = null;
  let groupBounds = { x: 0, y: 0, w: 0, h: 0 };

  ctx.clearRect(0, 0, buffer.viewport_width, buffer.viewport_height);

  for (const op of buffer.ops) {
    if (op === 'GroupEnd') {
      if (currentTaskId && groupBounds.w > 0 && groupBounds.h > 0) {
        hitRegions.push({
          taskId: currentTaskId,
          x: groupBounds.x,
          y: groupBounds.y,
          w: groupBounds.w,
          h: groupBounds.h,
        });
      }
      currentTaskId = null;
      groupBounds = { x: 0, y: 0, w: 0, h: 0 };
      continue;
    }

    if ('GroupStart' in op) {
      currentTaskId = op.GroupStart.task_id;
      groupBounds = { x: 0, y: 0, w: 0, h: 0 };
      continue;
    }

    if ('FillRect' in op) {
      const { x, y, w, h, color_id, radius } = op.FillRect;
      ctx.fillStyle = resolveColor(buffer, color_id);
      fillRoundedRect(ctx, x, y, w, h, radius);
      if (currentTaskId) {
        groupBounds = unionRect(groupBounds, x, y, w, h);
      }
      continue;
    }

    if ('FillPolygon' in op) {
      const { points, color_id } = op.FillPolygon;
      if (points.length === 0) continue;
      ctx.fillStyle = resolveColor(buffer, color_id);
      ctx.beginPath();
      ctx.moveTo(points[0][0], points[0][1]);
      for (let i = 1; i < points.length; i++) {
        ctx.lineTo(points[i][0], points[i][1]);
      }
      ctx.closePath();
      ctx.fill();
      if (currentTaskId && points.length > 0) {
        const xs = points.map((p) => p[0]);
        const ys = points.map((p) => p[1]);
        const minX = Math.min(...xs);
        const minY = Math.min(...ys);
        groupBounds = unionRect(
          groupBounds,
          minX,
          minY,
          Math.max(...xs) - minX,
          Math.max(...ys) - minY,
        );
      }
      continue;
    }

    if ('StrokePath' in op) {
      const { d, color_id, width } = op.StrokePath;
      applyStrokeStyle(ctx, buffer, color_id, width, null);
      const path = new Path2D(d);
      ctx.stroke(path);
      continue;
    }

    if ('StrokePolyline' in op) {
      const { points, color_id, width, dash } = op.StrokePolyline;
      if (points.length < 2) continue;
      applyStrokeStyle(ctx, buffer, color_id, width, dash);
      ctx.beginPath();
      ctx.moveTo(points[0][0], points[0][1]);
      for (let i = 1; i < points.length; i++) {
        ctx.lineTo(points[i][0], points[i][1]);
      }
      ctx.stroke();
      continue;
    }

    if ('DrawText' in op) {
      const { x, y, text, color_id, anchor, size, weight } = op.DrawText;
      ctx.fillStyle = resolveColor(buffer, color_id);
      ctx.font = `${weight} ${size}px sans-serif`;
      ctx.textBaseline = 'alphabetic';
      if (anchor === 1) ctx.textAlign = 'center';
      else if (anchor === 2) ctx.textAlign = 'end';
      else ctx.textAlign = 'start';
      ctx.fillText(text, x, y);
      ctx.textAlign = 'start';
    }
  }

  return { hitRegions };
}

export function parseCommandBuffer(json: string): CommandBuffer {
  return JSON.parse(json) as CommandBuffer;
}

export function findTaskAtPoint(
  hitRegions: TaskHitRegion[],
  x: number,
  y: number,
): string | null {
  for (let i = hitRegions.length - 1; i >= 0; i--) {
    const r = hitRegions[i];
    if (x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h) {
      return r.taskId;
    }
  }
  return null;
}
