import type { Meta, StoryObj } from '@storybook/vue3-vite';
import { action } from 'storybook/actions';
import GanttChart from './GanttChart.vue';
import type { GanttLane, GanttTask } from './types.ts';

// A chart in lanes: one lane per name, the bars of the tasks in it packed into rows, so
// bars that overlap in time sit one above the other. These stories are new; the stories
// of `GanttChart` itself are not changed, and a chart without `lanes` is drawn as before.
const meta: Meta<typeof GanttChart> = {
  component: GanttChart,
  title: 'GanttChart/Lanes',
};

export default meta;
type Story = StoryObj<typeof GanttChart>;

const vrtArgs = { deviceTier: 'high' as const, today: '2026-06-06' };

const render: Story['render'] = (args) => ({
  components: { GanttChart },
  setup() {
    return { args, onTaskClick: action('taskClick') };
  },
  template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
});

const twoLanes: GanttLane[] = [
  { id: 'alice', label: 'alice' },
  { id: 'bob', label: 'bob' },
];

export const TwoLanes: Story = {
  args: {
    ...vrtArgs,
    lanes: twoLanes,
    tasks: [
      { id: 'a1', title: 'Design', progress_pct: 100, start: '2026-06-01', end: '2026-06-04', lane_ids: ['alice'] },
      { id: 'a2', title: 'Implement', progress_pct: 60, start: '2026-06-05', end: '2026-06-11', lane_ids: ['alice'] },
      { id: 'b1', title: 'Review', progress_pct: 30, start: '2026-06-02', end: '2026-06-08', lane_ids: ['bob'] },
      { id: 'b2', title: 'Release', progress_pct: 0, start: '2026-06-09', end: '2026-06-12', lane_ids: ['bob'] },
    ],
  },
  render,
};

export const ThreeLanesWithUnassigned: Story = {
  args: {
    ...vrtArgs,
    lanes: [...twoLanes, { id: 'unassigned', label: 'Unassigned' }],
    tasks: [
      { id: 'a1', title: 'Design', progress_pct: 100, start: '2026-06-01', end: '2026-06-04', lane_ids: ['alice'] },
      { id: 'b1', title: 'Review', progress_pct: 30, start: '2026-06-02', end: '2026-06-08', lane_ids: ['bob'] },
      { id: 'u1', title: 'Investigate', progress_pct: 10, start: '2026-06-03', end: '2026-06-10', lane_ids: ['unassigned'] },
      { id: 'u2', title: 'Triage', progress_pct: 0, start: '2026-06-05', end: '2026-06-07', lane_ids: ['unassigned'] },
    ],
  },
  render,
};

// Three bars that overlap in time go into three rows of one lane; the fourth starts after
// the first has ended and goes back into the top row.
export const OverlappingBars: Story = {
  args: {
    ...vrtArgs,
    lanes: [{ id: 'alice', label: 'alice' }],
    tasks: [
      { id: 'o1', title: 'Spec', progress_pct: 80, start: '2026-06-01', end: '2026-06-05', lane_ids: ['alice'] },
      { id: 'o2', title: 'Prototype', progress_pct: 40, start: '2026-06-03', end: '2026-06-10', lane_ids: ['alice'] },
      { id: 'o3', title: 'Interview', progress_pct: 10, start: '2026-06-04', end: '2026-06-08', lane_ids: ['alice'] },
      { id: 'o4', title: 'Wrap up', progress_pct: 0, start: '2026-06-08', end: '2026-06-12', lane_ids: ['alice'] },
    ],
  },
  render,
};

// A task that two people share is drawn in both lanes. Clicking either bar emits the same task.
export const TaskInTwoLanes: Story = {
  args: {
    ...vrtArgs,
    lanes: twoLanes,
    tasks: [
      { id: 'shared', title: 'Pair on migration', progress_pct: 50, start: '2026-06-02', end: '2026-06-09', lane_ids: ['alice', 'bob'] },
      { id: 'a1', title: 'Docs', progress_pct: 20, start: '2026-06-03', end: '2026-06-06', lane_ids: ['alice'] },
      { id: 'b1', title: 'Tests', progress_pct: 70, start: '2026-06-04', end: '2026-06-07', lane_ids: ['bob'] },
    ],
  },
  render,
};

// A lane nobody belongs to keeps one row and its name. A task in none of the lanes is not drawn.
export const EmptyLane: Story = {
  args: {
    ...vrtArgs,
    lanes: [...twoLanes, { id: 'carol', label: 'carol' }],
    tasks: [
      { id: 'a1', title: 'Design', progress_pct: 100, start: '2026-06-01', end: '2026-06-04', lane_ids: ['alice'] },
      { id: 'b1', title: 'Review', progress_pct: 30, start: '2026-06-02', end: '2026-06-08', lane_ids: ['bob'] },
      { id: 'n1', title: 'Not in any lane', progress_pct: 0, start: '2026-06-02', end: '2026-06-08', lane_ids: [] },
    ],
  },
  render,
};

export const LanesOnCanvas: Story = {
  args: {
    ...vrtArgs,
    backend: 'canvas',
    lanes: twoLanes,
    tasks: [
      { id: 'shared', title: 'Pair on migration', progress_pct: 50, start: '2026-06-02', end: '2026-06-09', lane_ids: ['alice', 'bob'] },
      { id: 'a1', title: 'Docs', progress_pct: 20, start: '2026-06-03', end: '2026-06-06', lane_ids: ['alice'] },
      { id: 'b1', title: 'Tests', progress_pct: 70, start: '2026-06-04', end: '2026-06-07', lane_ids: ['bob'] },
    ],
  },
  render,
};

// Many tasks in a few lanes. A low-tier device draws only the lanes and rows a scroll shows,
// so this is the story that has virtualization at work. The tasks are made from their index
// alone, so the picture does not change from run to run.
const manyLanes: GanttLane[] = ['alice', 'bob', 'carol', 'dave', 'erin', 'frank'].map((id) => ({ id, label: id }));
const manyTasks: GanttTask[] = Array.from({ length: 120 }, (_, i) => {
  const start = (i * 3) % 40;
  const length = 2 + (i % 6);
  const day = (n: number) => `2026-${n < 30 ? '06' : '07'}-${String((n % 30) + 1).padStart(2, '0')}`;
  return {
    id: `m${i}`,
    title: `Task ${i}`,
    progress_pct: (i * 7) % 101,
    start: day(start),
    end: day(start + length),
    lane_ids: [manyLanes[i % manyLanes.length]!.id],
  };
});

export const ManyTasksVirtualized: Story = {
  args: {
    deviceTier: 'low',
    today: '2026-06-12',
    lanes: manyLanes,
    tasks: manyTasks,
  },
  render,
};
