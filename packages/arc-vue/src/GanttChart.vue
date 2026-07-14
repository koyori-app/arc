<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import init, { render_svg, render_canvas_commands } from '@koyori-app/arc';
import type { GanttTask, GanttDep } from './types.ts';
import {
  parseCommandBuffer,
  replayCommands,
  findTaskAtPoint,
  type TaskHitRegion,
} from './replayCommands';
import { resetCanvasElement } from './canvasLifecycle';

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

/** Mirrors koyori-arc-core display_list constants */
const ROW_H = 40;
const HEADER_H = 30;
const LEGEND_H = 40;

const ready = ref(false);
const svg = ref('');
const scrollY = ref(0);
const clientHeight = ref(600);
const scrollRef = ref<HTMLElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const hitRegions = ref<TaskHitRegion[]>([]);

const useCanvas = computed(() => props.backend === 'canvas');

onMounted(async () => {
  await init();
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
  if (props.tasks.length === 0) return 0;
  return props.tasks.length * ROW_H + HEADER_H + LEGEND_H + 10;
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

function resetCanvas(canvas: HTMLCanvasElement | null) {
  hitRegions.value = [];
  resetCanvasElement(canvas);
}

async function paintCanvas() {
  const canvas = canvasRef.value;
  const json = canvasCommandsJson.value;
  if (!canvas || !json) {
    resetCanvas(canvas);
    return;
  }

  const buffer = parseCommandBuffer(json);
  if (buffer.error) {
    resetCanvas(canvas);
    return;
  }

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
  <div class="koyori-gantt" @click="!useCanvas && onSvgClick($event)">
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
    :class="{ 'koyori-gantt-scroll--virtual': useVirtualization }"
    @scroll="onScroll"
  >
    <div
      class="koyori-gantt-inner"
      :style="{ height: `${chartHeight}px` }"
    >
      <canvas
        v-if="useCanvas"
        ref="canvasRef"
        class="koyori-gantt-canvas"
        role="img"
        aria-label="Gantt chart"
        @click="onCanvasClick"
      />
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div v-else class="koyori-gantt-svg" v-html="svg" />
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
