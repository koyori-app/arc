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

// Progress-line (イナズマ線) edge cases, anchored at today = 2026-06-06.
// The status line should encode schedule variance at "today". The fix keys on
// progress == 0, NOT on "start is in the future":
//   - behind:   started, low progress      → jags LEFT (behind). Keep.
//   - on-track: straddles today            → near the today vertical. Keep.
//   - future0:  starts later, 0% (ANOMALY) → not started & not due ⇒ zero variance ⇒
//               should sit ON the today line. Currently jags RIGHT, falsely "ahead".
//               This is the only point the 形状是正 fix moves.
//   - future5:  starts later, 5% (CONTROL) → real work done early ⇒ genuinely ahead.
//               Must STAY to the right; the fix must NOT touch progress > 0.
// Pins current behavior as a visual baseline: after the fix, future0 moves to the
// today line while future5 stays put, proving the correction is surgical.
const progressLineTasks: GanttTask[] = [
  { id: 'behind', title: 'Behind schedule', progress_pct: 20, start: '2026-06-02', end: '2026-06-10' },
  { id: 'ontrack', title: 'On track', progress_pct: 50, start: '2026-06-03', end: '2026-06-09' },
  { id: 'future0', title: 'Future, not started', progress_pct: 0, start: '2026-06-10', end: '2026-06-14' },
  { id: 'future5', title: 'Future, 5% done', progress_pct: 5, start: '2026-06-11', end: '2026-06-16' },
];

export const ProgressLineEdgeCases: Story = {
  args: {
    tasks: progressLineTasks,
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
