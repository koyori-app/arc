<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import init, {
  render_svg,
  render_svg_error,
  render_canvas_commands,
  empty_svg_markup,
} from '@koyori-app/arc';
import type { GanttTask, GanttDep } from './types.ts';
import {
  parseCommandBuffer,
  replayCommands,
  findTaskAtPoint,
  type TaskHitRegion,
} from './replayCommands';
import { resetCanvasElement } from './canvasLifecycle';
import {
  chartHeightForTaskCount,
  isCanvasCapacityError,
  resolveCanvasFailure,
} from './canvasFallback';
import { EMPTY_SVG_MARKUP, parseRenderError, type RenderFailure } from './wasmContract';

const props = defineProps<{
  tasks: GanttTask[];
  deps?: GanttDep[];
  today?: string; // ISO 8601 date string, e.g. "2026-06-16"
  /** Render backend — default `svg` preserves existing DOM projection. */
  backend?: 'svg' | 'canvas';
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

onMounted(async () => {
  await init();
  // Read once, before `ready` unblocks the computeds that compare against it.
  emptySvgMarkup.value = empty_svg_markup();
  ready.value = true;
  await nextTick();
  if (scrollRef.value) {
    clientHeight.value = scrollRef.value.clientHeight;
  }
});

function detectDeviceTier(): 'low' | 'high' {
  if (typeof navigator === 'undefined') return 'low';
  const nav = navigator as Navigator & { deviceMemory?: number };
  if (nav.deviceMemory === undefined) return 'low';
  if (nav.deviceMemory <= 4) return 'low';
  if (navigator.hardwareConcurrency <= 4) return 'low';
  return 'high';
}

const deviceTier = detectDeviceTier();
const useVirtualization = computed(() => deviceTier === 'low');

const chartHeight = computed(() => {
  if (displayError.value) return 0;
  return chartHeightForTaskCount(props.tasks.length);
});

const viewportJson = computed(() => {
  if (!useVirtualization.value) return undefined;
  return JSON.stringify({
    scroll_y: scrollY.value,
    client_height: clientHeight.value,
  });
});

const svgHtml = computed(() => {
  if (!ready.value || props.tasks.length === 0 || useCanvas.value) return '';
  return render_svg(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    props.today ?? undefined,
    viewportJson.value,
  );
});

const canvasCommandsJson = computed(() => {
  if (!ready.value || props.tasks.length === 0 || !useCanvas.value) return '';
  return render_canvas_commands(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    props.today ?? undefined,
    viewportJson.value,
  );
});

watch(svgHtml, (v) => { svg.value = v; }, { immediate: true });

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
  const reported = render_svg_error(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
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
      fallbackSvg = render_svg(
        JSON.stringify(props.tasks),
        JSON.stringify(props.deps ?? []),
        props.today ?? undefined,
        JSON.stringify({
          scroll_y: scrollY.value,
          client_height: Math.min(clientHeight.value, 600),
        }),
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

  canvas.width = buffer.viewport_width;
  canvas.height = buffer.viewport_height;
  canvas.style.width = `${buffer.viewport_width}px`;
  canvas.style.height = `${buffer.viewport_height}px`;

  const ctx = canvas.getContext('2d');
  if (!ctx) {
    resetCanvas(canvas);
    return;
  }
  const result = replayCommands(ctx, buffer);
  hitRegions.value = result.hitRegions;
}

watch(canvasCommandsJson, () => { void nextTick().then(paintCanvas); }, { immediate: true });

function onScroll(e: Event) {
  const el = e.target as HTMLElement;
  scrollY.value = el.scrollTop;
  clientHeight.value = el.clientHeight;
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
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width / rect.width;
  const scaleY = canvas.height / rect.height;
  const x = (e.clientX - rect.left) * scaleX;
  const y = (e.clientY - rect.top) * scaleY;
  const taskId = findTaskAtPoint(hitRegions.value, x, y);
  if (!taskId) return;
  const task = props.tasks.find((t) => t.id === taskId);
  if (task) emit('taskClick', task);
}
</script>

<template>
  <div class="koyori-gantt" @click="(!useCanvas || canvasFallbackSvg) && onSvgClick($event)">
    <div v-if="!ready" class="koyori-gantt-skeleton" aria-hidden="true">
      <div v-for="task in props.tasks" :key="task.id" class="koyori-gantt-skeleton-row">
        <div class="koyori-gantt-skeleton-label" />
        <div class="koyori-gantt-skeleton-bar" :style="{ width: `${task.progress_pct}%` }" />
      </div>
    </div>
  <div
    v-else
    ref="scrollRef"
    class="koyori-gantt-scroll"
    :class="{ 'koyori-gantt-scroll--virtual': useVirtualization || canvasFailure }"
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
/* Mirrors render.rs layout constants (ROW_H=40, LABEL_W=120, BAR_H=20) */
.koyori-gantt-skeleton-row {
  display: flex;
  align-items: center;
  height: 40px;
  gap: 4px;
}
.koyori-gantt-skeleton-label {
  width: 120px;
  height: 12px;
  border-radius: 4px;
  background: #e5e7eb;
  flex-shrink: 0;
}
.koyori-gantt-skeleton-bar {
  height: 20px;
  min-width: 8px;
  max-width: calc(100% - 132px);
  border-radius: 4px;
  background: #d1d5db;
  animation: koyori-gantt-shimmer 1.4s ease-in-out infinite;
}
@keyframes koyori-gantt-shimmer {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}
</style>
