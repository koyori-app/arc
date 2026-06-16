import type { Meta, StoryObj } from '@storybook/vue3-vite';
import { action } from 'storybook/actions';
import GanttChart from './GanttChart.vue';
import type { GanttDep, GanttTask } from './types.ts';

const tasks: GanttTask[] = [
  { id: 't1', title: 'Design', progress_pct: 100, start: '2026-06-01', end: '2026-06-04' },
  { id: 't2', title: 'Backend API', progress_pct: 60, start: '2026-06-02', end: '2026-06-08' },
  { id: 't3', title: 'Frontend', progress_pct: 30, start: '2026-06-04', end: '2026-06-10' },
  { id: 't4', title: 'QA', progress_pct: 0, start: '2026-06-09', end: '2026-06-12' },
];

const deps: GanttDep[] = [
  { blocker_task_id: 't1', blocked_task_id: 't2' },
  { blocker_task_id: 't1', blocked_task_id: 't3' },
  { blocker_task_id: 't2', blocked_task_id: 't4' },
  { blocker_task_id: 't3', blocked_task_id: 't4' },
];

const meta: Meta<typeof GanttChart> = {
  component: GanttChart,
  title: 'GanttChart',
};

export default meta;
type Story = StoryObj<typeof GanttChart>;

export const Default: Story = {
  args: {
    tasks,
    deps,
    today: '2026-06-06',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

export const Empty: Story = {
  args: {
    tasks: [],
  },
};

export const NoDependencies: Story = {
  args: {
    tasks,
  },
};
