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
    <!-- eslint-disable-next-line vue/no-v-html -->
    <div v-html="svg" />
  </div>
</template>
