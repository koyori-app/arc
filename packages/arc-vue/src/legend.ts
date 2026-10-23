import init, { legend_spec_json } from '@koyori-app/arc';

/**
 * What the legend says, in the shape `koyori_arc_core::LegendSpec` serializes to.
 * Only the shape is written here. The values (names, colours, ranges, stroke)
 * are read from the loaded Wasm by `loadLegendSpec()`, never copied into this
 * package: arc-vue and `@koyori-app/arc` are installed at versions that can
 * differ, and a copy cannot see that.
 */
export interface LegendTier {
  /** Stable machine name of the tier, e.g. a class suffix. */
  key: string;
  label: string;
  /** A CSS colour. */
  color: string;
  /** Lowest `progress_pct` that falls in this tier. */
  min_pct: number;
  /** Highest `progress_pct` that falls in this tier. */
  max_pct: number;
}

export interface LegendProgressLine {
  label: string;
  /** A CSS colour. */
  color: string;
  stroke_width: number;
  /** An SVG `stroke-dasharray` value. */
  dash: string;
}

export interface LegendSpec {
  tiers: LegendTier[];
  progress_line: LegendProgressLine;
}

/**
 * The legend a chart draws, for a caller that hides it (`legend="hidden"`) and
 * draws its own. The Wasm module is initialized first, so this can be called
 * before any chart has mounted.
 */
export async function loadLegendSpec(): Promise<LegendSpec> {
  await init();
  return JSON.parse(legend_spec_json()) as LegendSpec;
}
