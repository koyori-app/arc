/** Browser ES module — mirrors packages/arc-vue/src/replayCommands.ts for bench harness. */

const COLOR_ID_NAMES = [
  'BarBg', 'TierLow', 'TierMid', 'TierHigh', 'TierDone', 'Dep', 'Progress',
  'Grid', 'Today', 'HeaderBg', 'GridLabel', 'ProgressTextOnFg', 'ProgressTextOnBg',
];

function resolveColor(buffer, colorId) {
  if (colorId === 255) return '#374151';
  const name = COLOR_ID_NAMES[colorId];
  if (!name) return '#000000';
  const entry = buffer.palette.colors.find(([id]) => id === name);
  return entry?.[1] ?? '#000000';
}

function unionRect(r, x, y, w, h) {
  if (w <= 0 || h <= 0) return r;
  const x2 = x + w;
  const y2 = y + h;
  if (r.w === 0 && r.h === 0) return { x, y, w, h };
  const nx = Math.min(r.x, x);
  const ny = Math.min(r.y, y);
  return { x: nx, y: ny, w: Math.max(r.x + r.w, x2) - nx, h: Math.max(r.y + r.h, y2) - ny };
}

function fillRoundedRect(ctx, x, y, w, h, radius) {
  if (radius > 0 && typeof ctx.roundRect === 'function') {
    ctx.beginPath();
    ctx.roundRect(x, y, w, h, radius);
    ctx.fill();
    return;
  }
  ctx.fillRect(x, y, w, h);
}

function applyStrokeStyle(ctx, buffer, colorId, width, dash) {
  ctx.strokeStyle = resolveColor(buffer, colorId);
  ctx.lineWidth = width;
  if (dash) {
    const parts = dash.split(',').map((s) => Number.parseFloat(s.trim()));
    ctx.setLineDash(parts.every(Number.isFinite) ? parts : []);
  } else {
    ctx.setLineDash([]);
  }
}

export function replayCommands(ctx, buffer) {
  let currentTaskId = null;
  let groupBounds = { x: 0, y: 0, w: 0, h: 0 };
  ctx.clearRect(0, 0, buffer.viewport_width, buffer.viewport_height);

  for (const op of buffer.ops) {
    if (op === 'GroupEnd') {
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
      if (currentTaskId) groupBounds = unionRect(groupBounds, x, y, w, h);
      continue;
    }
    if ('FillPolygon' in op) {
      const { points, color_id } = op.FillPolygon;
      if (points.length === 0) continue;
      ctx.fillStyle = resolveColor(buffer, color_id);
      ctx.beginPath();
      ctx.moveTo(points[0][0], points[0][1]);
      for (let i = 1; i < points.length; i++) ctx.lineTo(points[i][0], points[i][1]);
      ctx.closePath();
      ctx.fill();
      continue;
    }
    if ('StrokePath' in op) {
      const { d, color_id, width } = op.StrokePath;
      applyStrokeStyle(ctx, buffer, color_id, width, null);
      ctx.stroke(new Path2D(d));
      continue;
    }
    if ('StrokePolyline' in op) {
      const { points, color_id, width, dash } = op.StrokePolyline;
      if (points.length < 2) continue;
      applyStrokeStyle(ctx, buffer, color_id, width, dash);
      ctx.beginPath();
      ctx.moveTo(points[0][0], points[0][1]);
      for (let i = 1; i < points.length; i++) ctx.lineTo(points[i][0], points[i][1]);
      ctx.stroke();
      continue;
    }
    if ('DrawText' in op) {
      const { x, y, text, color_id, anchor, size, weight } = op.DrawText;
      ctx.fillStyle = resolveColor(buffer, color_id);
      ctx.font = `${weight} ${size}px sans-serif`;
      ctx.textBaseline = 'alphabetic';
      ctx.textAlign = anchor === 1 ? 'center' : anchor === 2 ? 'end' : 'start';
      ctx.fillText(text, x, y);
      ctx.textAlign = 'start';
    }
  }
}

export function parseCommandBuffer(json) {
  return JSON.parse(json);
}
