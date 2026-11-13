export { default as GanttChart } from './GanttChart.vue';
export type { GanttTask, GanttDep, GanttRow } from './types.ts';
export {
  replayCommands,
  parseCommandBuffer,
  findTaskAtPoint,
} from './replayCommands';
export type {
  ColorIdName,
  CommandBuffer,
  DrawOp,
  ReplayResult,
  TaskHitRegion,
} from './replayCommands';
export { loadLegendSpec } from './legend';
export type { LegendSpec, LegendTier, LegendProgressLine } from './legend';
export type { BarAlign, ChartOptions, LabelMode, LegendMode, PaletteOverrides } from './options';
