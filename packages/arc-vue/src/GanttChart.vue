<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import init, { render_svg } from '@koyori-app/arc';
import type { GanttTask, GanttDep } from './types.ts';

const props = defineProps<{
  tasks: GanttTask[];
  deps?: GanttDep[];
  today?: string; // ISO 8601 date string, e.g. "2026-06-16"
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
  if (!ready.value || props.tasks.length === 0) return '';
  return render_svg(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    props.today ?? undefined,
    viewportJson.value,
  );
});

watch(svgHtml, (v) => { svg.value = v; }, { immediate: true });

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
</script>

<template>
  <div class="koyori-gantt" @click="onSvgClick">
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
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div class="koyori-gantt-svg" v-html="svg" />
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
