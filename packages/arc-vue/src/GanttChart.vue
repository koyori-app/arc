<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
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

const ready = ref(false);
const svg = ref('');

onMounted(async () => {
  await init();
  ready.value = true;
});

const svgHtml = computed(() => {
  if (!ready.value || props.tasks.length === 0) return '';
  return render_svg(
    JSON.stringify(props.tasks),
    JSON.stringify(props.deps ?? []),
    props.today ?? undefined,
  );
});

watch(svgHtml, (v) => { svg.value = v; }, { immediate: true });

function onSvgClick(e: MouseEvent) {
  // task bars will carry data-task-id once render_svg emits them
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
    <!-- eslint-disable-next-line vue/no-v-html -->
    <div v-else v-html="svg" />
  </div>
</template>

<style scoped>
/* Mirrors render.rs layout constants (ROW_H=40, LABEL_W=120, BAR_H=20)
   so the skeleton doesn't jump when the real SVG mounts. */
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
