export { default as GanttChart } from './GanttChart.vue';
export type { GanttTask, GanttDep } from './types.ts';
export {
  replayCommands,
  parseCommandBuffer,
  findTaskAtPoint,
} from './replayCommands';
export type { CommandBuffer, DrawOp, ReplayResult, TaskHitRegion } from './replayCommands';
export { loadLegendSpec } from './legend';
export type { LegendSpec, LegendTier, LegendProgressLine } from './legend';
export type { ChartOptions, LegendMode } from './options';
