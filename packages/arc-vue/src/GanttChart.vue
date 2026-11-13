<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue';
import init, {
  render_svg,
  render_svg_error,
  render_canvas_commands,
  empty_svg_markup,
} from '@koyori-app/arc';
import type { GanttTask, GanttDep, GanttRow } from './types.ts';
import {
  parseCommandBuffer,
  replayCommands,
  findTaskAtPoint,
  type TaskHitRegion,
} from './replayCommands';
import { resetCanvasElement } from './canvasLifecycle';
import { buildOptionsJson, type PaletteOverrides } from './options';
import { backingSize, cssPointFromClient } from './canvasScale';
import {
  chartHeightForRows,
  isCanvasCapacityError,
  resolveCanvasFailure,
  skeletonLabelWidthPx,
} from './canvasFallback';
import {
  EMPTY_SVG_MARKUP,
  RUST_LAYOUT,
  parseRenderError,
  type RenderFailure,
} from './wasmContract';

/**
 * The loading skeleton mimics a real chart row, so its metrics are the crate's.
 * `v-bind` in `<style>` is what keeps them from being retyped as CSS literals:
 * these are the same numbers `wasmContract.test.ts` pins to
 * `display_list/constants.rs`, not a second set that happens to match today.
 */
const skeletonRowH = computed(() => `${props.rowHeight ?? RUST_LAYOUT.ROW_H}px`);
// A chart without the title column has no label slot in its skeleton either,
// only the left margin the chart keeps for a first-day milestone. This width
// is the one the tests pin, so it is set inline on the skeleton as a custom
// property instead of through `v-bind` in `<style>`: the Node build of Vue
// that the tests load makes `useCssVars` a no-op, and a value the tests
// cannot read is a value they cannot pin (GanttChart.skeleton.test.ts).
const skeletonLabelW = computed(
  () => `${skeletonLabelWidthPx(props.rowLabels, props.barHeight)}px`,
);
const skeletonBarH = `${RUST_LAYOUT.BAR_H}px`;

const props = defineProps<{
  tasks: GanttTask[];
  deps?: GanttDep[];
  today?: string; // ISO 8601 date ("2026-06-16") or date-time ("2026-06-16T09:00:00+09:00")
  /**
   * The viewer's time zone as minutes east of UTC (JST is 540). Date-time
   * strings that carry an offset (`start`, `end`, `today`) are converted to this
   * zone's calendar day; date-only strings never are. Left out, the day as
   * written is used. It is never read from the browser: the caller says it, so
   * the same data draws the same days for every viewer (see SPEC.md).
   */
  tzOffsetMinutes?: number;
  /**
   * `'hidden'` leaves the colour legend out and takes its height off the
   * chart, for a caller that draws its own (see `loadLegendSpec`). Left out,
   * the legend is drawn as before. It is a string and not a boolean on purpose:
   * Vue turns an omitted boolean prop into `false`, which here would hide the
   * legend of every existing caller.
   */
  legend?: 'shown' | 'hidden';
  /**
   * `'hidden'` leaves the title column out: the days start half a bar
   * (`HIDDEN_LABELS_LEFT_PX`, or half of `barHeight` when it is set) in from
   * the left edge, and a click hits only the bars and the milestones' diamonds.
   * For a caller that lays the chart beside a list of its own. A string for the
   * same reason as `legend`.
   */
  rowLabels?: 'shown' | 'hidden';
  /** `'hidden'` leaves out the `NN%` on every bar and milestone. */
  percentLabels?: 'shown' | 'hidden';
  /**
   * The chart's rows, top to bottom, for a caller that lays them out itself
   * beside a list of its own: task rows and group rows (a band, no bar). A
   * task left out is not drawn. Left out, one row per task as the crate lays
   * them out. See `docs/design/external-rows.md`.
   */
  rows?: GanttRow[];
  /** Height of a row in pixels. Left out, the crate's own. */
  rowHeight?: number;
  /** Height of the date header in pixels. Left out, the crate's own. */
  headerHeight?: number;
  /** Height of a bar in pixels. Left out, the crate's own. */
  barHeight?: number;
  /** `'bottom'` sets a bar low in a tall row, leaving the room above it. */
  barAlign?: 'middle' | 'bottom';
  /**
   * Colours that replace the chart's own, by colour name, such as
   * `{ HeaderBg: 'var(--koyori-color-surface)' }`. The SVG path writes
   * `var(--…)` as it is, and the browser follows a token whenever it changes.
   * The canvas path reads it off the canvas's computed style when it paints,
   * and paints again when the colour scheme or an ancestor's `class`, `style`
   * or `data-*` changes. A token changed any other way (a stylesheet swapped
   * in, say) needs `repaint()`, or the palette passed again as a new object.
   */
  palette?: PaletteOverrides;
  /** Render backend — default `svg` preserves existing DOM projection. */
  backend?: 'svg' | 'canvas';
  /** Override auto-detected device tier (VRT / Storybook should pin this). */
  deviceTier?: 'low' | 'high';
}>();

const emit = defineEmits<{
  taskClick: [task: GanttTask];
}>();

const ready = ref(false);
const svg = ref('');
const scrollY = ref(0);
const clientHeight = ref(600);
const scrollRef = ref<HTMLElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const hitRegions = ref<TaskHitRegion[]>([]);
const canvasFallbackSvg = ref('');
const canvasError = ref('');
const canvasFailure = ref<RenderFailure | null>(null);
/** 描いた canvas の CSS の寸法。当たり判定はこの座標で押した点を照らす。 */
let canvasCssSize = { width: 0, height: 0 };
/**
 * The empty-chart markup of the `@koyori-app/arc` build actually loaded.
 *
 * Two checks that look redundant answer different questions, so keep both:
 * - `EMPTY_SVG_MARKUP` in `wasmContract.ts` is checked against the Rust source
 *   by `wasmContract.test.ts`. That catches drift *inside this repo*, at build
 *   time, without needing a Wasm build.
 * - This ref is what every *runtime* comparison uses. `arc-vue` and
 *   `@koyori-app/arc` ship as separate packages and can be installed at
 *   different versions, and no test in this repo can see that pairing. Asking
 *   the loaded module is the only way to be right about it.
 *
 * Deleting either one because "it is already covered" reopens one of the two.
 */
const emptySvgMarkup = ref(EMPTY_SVG_MARKUP);

const useCanvas = computed(() => props.backend === 'canvas');

function syncScrollViewport(el?: HTMLElement | null) {
  const scrollEl = el ?? scrollRef.value;
  if (!scrollEl) return;
  scrollY.value = scrollEl.scrollTop;
  clientHeight.value = scrollEl.clientHeight;
}

onMounted(async () => {
  await init();
  // Read once, before `ready` unblocks the computeds that compare against it.
  emptySvgMarkup.value = empty_svg_markup();
  ready.value = true;
  await nextTick();
  syncScrollViewport();
  if (requestedVirtualization.value) {
    viewportReady.value = true;
  }
});

function detectDeviceTier(): 'low' | 'high' {
  if (props.deviceTier) return props.deviceTier;
  if (typeof navigator === 'undefined') return 'low';
  const nav = navigator as Navigator & { deviceMemory?: number };
  if (nav.deviceMemory === undefined) return 'low';
  if (nav.deviceMemory <= 4) return 'low';
  if (navigator.hardwareConcurrency <= 4) return 'low';
  return 'high';
}

const deviceTier = computed(() => detectDeviceTier());
/** CSS class: requested virtualization state (does not wait for DOM measure). */
const requestedVirtualization = computed(() => deviceTier.value === 'low');
/** WASM viewport: only after DOM reflects CSS and scroll metrics are synced. */
const viewportReady = ref(false);

watch(requestedVirtualization, async (virtual) => {
  if (!virtual) {
    viewportReady.value = false;
    return;
  }
  viewportReady.value = false;
  await nextTick();
  syncScrollViewport();
  viewportReady.value = true;
}, { flush: 'post' });

const chartHeight = computed(() => {
  if (displayError.value) return 0;
  return chartHeightForRows(props.rows, props.tasks.length, props.legend, {
    rowHeight: props.rowHeight,
    headerHeight: props.headerHeight,
  });
});

const scrollViewport = computed(() => {
  if (!requestedVirtualization.value || !viewportReady.value) return undefined;
  return {
    scroll_y: scrollY.value,
    client_height: clientHeight.value,
  };
});

/** Every option but the scroll window, which the two paths set differently. */
function chartOptionsJson(scroll: { scroll_y: number; client_height: number } | undefined) {
  return buildOptionsJson({
    today: props.today,
    scrollViewport: scroll,
    tzOffsetMinutes: props.tzOffsetMinutes,
    legend: props.legend,
    rowLabels: props.rowLabels,
    percentLabels: props.percentLabels,
    rowHeightPx: props.rowHeight,
    headerHeightPx: props.headerHeight,
    barHeightPx: props.barHeight,
    barAlign: props.barAlign,
    palette: props.palette,
  });
}

const optionsJson = computed(() => chartOptionsJson(scrollViewport.value));

/**
 * The trailing `rows_json` argument, passed only when the caller lays out the
 * rows. Left out, the entry points are called with the three arguments they
 * took before rows existed.
 */
const rowsArgs = computed<[] | [string]>(() =>
  props.rows ? [JSON.stringify(props.rows)] : [],
);

/**
 * Single chokepoint for both WASM entry points: while virtualization is
 * requested but the viewport is not yet measured (initial mount, tier-flip
 * window), defer the render instead of issuing a full-range WASM call.
 * Non-virtual (high tier) renders are never delayed.
 */
const viewportPending = computed(
  () => requestedVirtualization.value && !viewportReady.value,
);

const svgHtml = computed<string | null>(() => {
  if (!ready.value || props.tasks.length === 0 || useCanvas.value) return '';
  if (viewportPending.value) return null;
  return render_svg(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    optionsJson.value,
    ...rowsArgs.value,
  );
});

const canvasCommandsJson = computed(() => {
  if (!ready.value || props.tasks.length === 0 || !useCanvas.value) return '';
  if (viewportPending.value) return '';
  return render_canvas_commands(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    optionsJson.value,
    ...rowsArgs.value,
  );
});

// null = deferred (viewport pending): keep the last painted SVG, no blanking.
watch(svgHtml, (v) => { if (v !== null) svg.value = v; }, { immediate: true });

/**
 * `render_svg` answers a blank chart to both "no tasks" and "input refused".
 * `render_svg_error` is the channel that tells them apart, so a refusal reaches
 * the same role="alert" the Canvas path already uses instead of silently
 * showing an empty chart.
 */
const svgFailure = computed<RenderFailure | null>(() => {
  if (!ready.value || props.tasks.length === 0 || useCanvas.value) return null;
  // Only a blank chart can be hiding a refusal, and `render_svg` returns this
  // exact markup when it refuses — so the happy path never pays a second parse.
  if (svgHtml.value !== emptySvgMarkup.value) return null;
  // The same options are passed too: an unreadable `today` or offset refuses
  // the render the same way an unreadable task does, and only this channel says so.
  const reported = render_svg_error(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    optionsJson.value,
    ...rowsArgs.value,
  );
  return reported ? parseRenderError(reported) : null;
});

const displayError = computed(() => canvasError.value || svgFailure.value?.message || '');

function resetCanvas(canvas: HTMLCanvasElement | null) {
  hitRegions.value = [];
  resetCanvasElement(canvas);
}

function clearCanvasFailure() {
  canvasFallbackSvg.value = '';
  canvasError.value = '';
  canvasFailure.value = null;
}

function renderCanvasFallback(failure: RenderFailure) {
  canvasFailure.value = failure;
  let fallbackSvg = '';
  if (isCanvasCapacityError(failure)) {
    try {
      // Every option goes with it: left out, the SVG the canvas falls back to
      // would bring the legend back or lay out other rows, and not match the
      // height reserved for it.
      fallbackSvg = render_svg(
        JSON.stringify(props.tasks),
        JSON.stringify(props.deps ?? []),
        chartOptionsJson({
          scroll_y: scrollY.value,
          client_height: Math.min(clientHeight.value, 600),
        }),
        ...rowsArgs.value,
      );
    } catch {
      // resolveCanvasFailure turns a failed fallback into a visible error.
    }
  }
  const resolution = resolveCanvasFailure(failure, fallbackSvg, emptySvgMarkup.value);
  if (resolution.mode === 'svg') {
    canvasFallbackSvg.value = resolution.svg;
    canvasError.value = '';
  } else {
    canvasFallbackSvg.value = '';
    canvasError.value = resolution.message;
  }
}

watch([scrollY, clientHeight], () => {
  if (canvasFallbackSvg.value && canvasFailure.value) {
    renderCanvasFallback(canvasFailure.value);
  }
});

async function paintCanvas() {
  const json = canvasCommandsJson.value;
  if (!json) {
    resetCanvas(canvasRef.value);
    clearCanvasFailure();
    return;
  }

  let canvas = canvasRef.value;
  if (!canvas) {
    // A fallback/error replaces the canvas with v-if. Clear it first, then
    // wait for Vue to mount a fresh canvas before replaying recovered output.
    clearCanvasFailure();
    await nextTick();
    canvas = canvasRef.value;
    if (!canvas) return;
  }

  const buffer = parseCommandBuffer(json);
  if (buffer.error) {
    resetCanvas(canvas);
    renderCanvasFallback({ message: buffer.error, code: buffer.code });
    return;
  }

  clearCanvasFailure();

  // 裏打ちは devicePixelRatio 倍にし、CSS の寸法は変えない。描画の命令は CSS の
  // 座標のままなので、setTransform で裏打ちの倍率を掛けて合わせる。
  const cssWidth = buffer.viewport_width;
  const cssHeight = buffer.viewport_height;
  const backing = backingSize(cssWidth, cssHeight, pixelRatio.value);
  canvas.width = backing.width;
  canvas.height = backing.height;
  canvas.style.width = `${cssWidth}px`;
  canvas.style.height = `${cssHeight}px`;
  canvasCssSize = { width: cssWidth, height: cssHeight };

  const ctx = canvas.getContext('2d');
  if (!ctx) {
    resetCanvas(canvas);
    return;
  }
  if (cssWidth > 0 && cssHeight > 0) {
    ctx.setTransform(backing.width / cssWidth, 0, 0, backing.height / cssHeight, 0, 0);
  }
  const result = replayCommands(ctx, buffer);
  hitRegions.value = result.hitRegions;
}

watch(canvasCommandsJson, () => { void nextTick().then(paintCanvas); }, { immediate: true });

/**
 * devicePixelRatio が途中で変わったら描き直す。窓を倍率の違う画面へ移した時や、
 * 拡大率を変えた時に変わる。追わないと、変わる前の倍率の裏打ちが引き伸ばされ、
 * 直す前と同じくぼやける。変わるのは稀で、描き直しは一回で済む。
 * 変化は `(resolution: Ndppx)` の matchMedia で知る。一致が外れたら今の値で
 * 問い直し、次の変化を待つ。
 */
const pixelRatio = ref(currentPixelRatio());
let pixelRatioQuery: MediaQueryList | null = null;

function currentPixelRatio(): number {
  if (typeof window === 'undefined') return 1;
  return window.devicePixelRatio || 1;
}

function stopWatchingPixelRatio() {
  pixelRatioQuery?.removeEventListener('change', onPixelRatioChange);
  pixelRatioQuery = null;
}

function watchPixelRatio() {
  stopWatchingPixelRatio();
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
  pixelRatioQuery = window.matchMedia(`(resolution: ${currentPixelRatio()}dppx)`);
  pixelRatioQuery.addEventListener('change', onPixelRatioChange);
}

function onPixelRatioChange() {
  pixelRatio.value = currentPixelRatio();
  watchPixelRatio();
}

onMounted(watchPixelRatio);
onBeforeUnmount(stopWatchingPixelRatio);

watch(pixelRatio, () => {
  if (canvasRef.value && canvasCommandsJson.value) void paintCanvas();
});

/**
 * A palette colour written as `var(--name)` is read off the canvas's computed
 * style once, when the canvas is painted; the SVG path writes it as it is and
 * the browser follows the token. So while the canvas paints a palette that
 * names a `var()`, watch what a theme switch usually changes and paint again:
 * the `prefers-color-scheme` query, and the `class`, `style` and `data-*`
 * attributes of every ancestor of the chart up to `<html>` (a theme is put on
 * `<html>`, `<body>` or a wrapper, and a custom property is inherited only
 * from the ancestors). Each ancestor is watched on its own attributes, not its
 * subtree, so changes elsewhere in the page cost nothing. A token changed
 * another way is not seen: `repaint()`, or the palette passed again as a new
 * object, paints with the value it has now. Paints are gathered into one frame.
 */
const rootRef = ref<HTMLElement | null>(null);
const paletteNamesVar = computed(() =>
  Object.values(props.palette ?? {}).some((c) => typeof c === 'string' && c.includes('var(')),
);
const paintsTokenColors = computed(
  () => useCanvas.value && !canvasFallbackSvg.value && paletteNamesVar.value,
);
let themeQuery: MediaQueryList | null = null;
let themeObserver: MutationObserver | null = null;
let repaintFrame = 0;

/** Paints the canvas again, once per frame however often it is asked. */
function repaint() {
  if (repaintFrame) return;
  const paint = () => {
    repaintFrame = 0;
    if (canvasRef.value && canvasCommandsJson.value) void paintCanvas();
  };
  if (typeof requestAnimationFrame === 'function') {
    repaintFrame = requestAnimationFrame(paint);
  } else {
    paint();
  }
}

function isThemeAttribute(name: string | null): boolean {
  return name === 'class' || name === 'style' || (name?.startsWith('data-') ?? false);
}

function stopWatchingTheme() {
  themeQuery?.removeEventListener('change', repaint);
  themeQuery = null;
  themeObserver?.disconnect();
  themeObserver = null;
}

function watchTheme() {
  stopWatchingTheme();
  if (typeof window === 'undefined' || !paintsTokenColors.value) return;
  if (typeof window.matchMedia === 'function') {
    themeQuery = window.matchMedia('(prefers-color-scheme: dark)');
    themeQuery.addEventListener('change', repaint);
  }
  if (typeof MutationObserver !== 'undefined' && rootRef.value) {
    themeObserver = new MutationObserver((records) => {
      if (records.some((r) => isThemeAttribute(r.attributeName))) repaint();
    });
    for (let el = rootRef.value.parentElement; el; el = el.parentElement) {
      themeObserver.observe(el, { attributes: true });
    }
  }
}

onMounted(watchTheme);
watch(paintsTokenColors, watchTheme);
// A new palette object, even with the same colours, paints with the tokens' values now.
watch(
  () => props.palette,
  () => {
    watchTheme();
    if (useCanvas.value) repaint();
  },
);
onBeforeUnmount(() => {
  stopWatchingTheme();
  if (repaintFrame && typeof cancelAnimationFrame === 'function') {
    cancelAnimationFrame(repaintFrame);
  }
  repaintFrame = 0;
});

defineExpose({ repaint });

function onScroll(e: Event) {
  syncScrollViewport(e.target as HTMLElement);
}

function onSvgClick(e: MouseEvent) {
  const el = (e.target as Element).closest('[data-task-id]');
  if (!el) return;
  const id = el.getAttribute('data-task-id');
  const task = props.tasks.find((t) => t.id === id);
  if (task) emit('taskClick', task);
}

function onCanvasClick(e: MouseEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  // 当たり判定の領域は CSS の座標なので、裏打ちの寸法ではなく CSS の寸法へ戻す。
  const point = cssPointFromClient(
    e.clientX,
    e.clientY,
    canvas.getBoundingClientRect(),
    canvasCssSize.width,
    canvasCssSize.height,
  );
  if (!point) return;
  const taskId = findTaskAtPoint(hitRegions.value, point.x, point.y);
  if (!taskId) return;
  const task = props.tasks.find((t) => t.id === taskId);
  if (task) emit('taskClick', task);
}
</script>

<template>
  <div
    ref="rootRef"
    class="koyori-gantt"
    @click="(!useCanvas || canvasFallbackSvg) && onSvgClick($event)"
  >
    <div
      v-if="!ready"
      class="koyori-gantt-skeleton"
      :style="{ '--koyori-skeleton-label-w': skeletonLabelW }"
      aria-hidden="true"
    >
      <div v-for="task in props.tasks" :key="task.id" class="koyori-gantt-skeleton-row">
        <div class="koyori-gantt-skeleton-label" />
        <div class="koyori-gantt-skeleton-bar" :style="{ width: `${task.progress_pct}%` }" />
      </div>
    </div>
  <div
    v-else
    ref="scrollRef"
    class="koyori-gantt-scroll"
    :class="{ 'koyori-gantt-scroll--virtual': requestedVirtualization || canvasFailure }"
    @scroll="onScroll"
  >
    <div v-if="displayError" class="koyori-gantt-error" role="alert">
      Unable to render Gantt chart: {{ displayError }}
    </div>
    <div
      v-else
      class="koyori-gantt-inner"
      :style="{ height: `${chartHeight}px` }"
    >
      <canvas
        v-if="useCanvas && !canvasFallbackSvg"
        ref="canvasRef"
        class="koyori-gantt-canvas"
        role="img"
        aria-label="Gantt chart"
        @click="onCanvasClick"
      />
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div v-else class="koyori-gantt-svg" v-html="canvasFallbackSvg || svg" />
    </div>
  </div>
  </div>
</template>

<style scoped>
.koyori-gantt-scroll {
  width: 100%;
  overflow: auto;
}
.koyori-gantt-scroll--virtual {
  max-height: min(70vh, 600px);
}
.koyori-gantt-inner {
  position: relative;
  width: 100%;
}
.koyori-gantt-error {
  padding: 12px;
  color: #991b1b;
}
.koyori-gantt-svg {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
}
.koyori-gantt-canvas {
  position: absolute;
  top: 0;
  left: 0;
  display: block;
}
.koyori-gantt-svg :deep(svg) {
  display: block;
}
/* Row metrics come from the crate via v-bind — see skeletonRowH above.
   The gap and the bar's minimum width are the skeleton's own, so they are
   named here once and the bar's max-width subtracts the names, never a
   hand-added total. */
.koyori-gantt-skeleton {
  --koyori-skeleton-gap: 4px;
  --koyori-skeleton-bar-min-w: 8px;
}
.koyori-gantt-skeleton-row {
  display: flex;
  align-items: center;
  height: v-bind(skeletonRowH);
  gap: var(--koyori-skeleton-gap);
}
.koyori-gantt-skeleton-label {
  width: var(--koyori-skeleton-label-w);
  height: 12px;
  border-radius: 4px;
  background: #e5e7eb;
  flex-shrink: 0;
}
.koyori-gantt-skeleton-bar {
  height: v-bind(skeletonBarH);
  min-width: var(--koyori-skeleton-bar-min-w);
  max-width: calc(
    100% - var(--koyori-skeleton-label-w) - var(--koyori-skeleton-gap)
      - var(--koyori-skeleton-bar-min-w)
  );
  border-radius: 4px;
  background: #d1d5db;
  animation: koyori-gantt-shimmer 1.4s ease-in-out infinite;
}
@keyframes koyori-gantt-shimmer {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}
</style>
