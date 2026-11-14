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

const vrtArgs = { deviceTier: 'high' as const };

export const Default: Story = {
  args: {
    ...vrtArgs,
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
    ...vrtArgs,
    tasks: [],
  },
};

export const NoDependencies: Story = {
  args: {
    ...vrtArgs,
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
    ...vrtArgs,
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

export const CanvasBackend: Story = {
  args: {
    ...vrtArgs,
    tasks,
    deps,
    today: '2026-06-06',
    backend: 'canvas',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

// A caller that hides the legend draws its own (see `loadLegendSpec`). The
// chart loses the band the legend took, so these two pin the shorter figure on
// each backend. The stories above are not changed.
export const LegendHidden: Story = {
  args: {
    ...vrtArgs,
    tasks,
    deps,
    today: '2026-06-06',
    legend: 'hidden',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

export const LegendHiddenCanvas: Story = {
  args: {
    ...vrtArgs,
    tasks,
    deps,
    today: '2026-06-06',
    legend: 'hidden',
    backend: 'canvas',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

// A caller that lays the chart beside its own list hides the title column and
// the percent labels, and usually the legend too. The days start half a bar
// in from the left edge of the chart.
export const LabelsHidden: Story = {
  args: {
    ...vrtArgs,
    tasks,
    deps,
    today: '2026-06-06',
    legend: 'hidden',
    rowLabels: 'hidden',
    percentLabels: 'hidden',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

export const LabelsHiddenCanvas: Story = {
  args: {
    ...vrtArgs,
    tasks,
    deps,
    today: '2026-06-06',
    legend: 'hidden',
    rowLabels: 'hidden',
    percentLabels: 'hidden',
    backend: 'canvas',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

// A caller that lays out its own rows beside its own list: group rows head the
// groups, the rows and the bars are its own sizes, and the bars sit low in
// their rows. The title column, the percent labels and the legend are off,
// as that caller draws them.
const externalRows = [
  { kind: 'group' as const },
  ...tasks.slice(0, 2).map((t) => ({ kind: 'task' as const, task_id: t.id })),
  { kind: 'group' as const },
  ...tasks.slice(2).map((t) => ({ kind: 'task' as const, task_id: t.id, height: 52 })),
];

export const ExternalRows: Story = {
  args: {
    ...vrtArgs,
    tasks,
    deps,
    today: '2026-06-06',
    legend: 'hidden',
    rowLabels: 'hidden',
    percentLabels: 'hidden',
    rows: externalRows,
    rowHeight: 40,
    headerHeight: 48,
    barHeight: 18,
    barAlign: 'bottom',
  },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, onTaskClick: action('taskClick') };
    },
    template: '<GanttChart v-bind="args" @task-click="onTaskClick" />',
  }),
};

export const ExternalRowsCanvas: Story = {
  args: { ...ExternalRows.args, backend: 'canvas' },
  render: ExternalRows.render,
};

// A caller that draws with its own design tokens: the colours are `var(--…)`
// of tokens set on a wrapper, and the canvas resolves them off its style.
const tokenStyle = [
  '--demo-surface: #faf8f5',
  '--demo-surface-hover: #f1edf7',
  '--demo-accent: #7c5cbf',
  '--demo-border: #e4dfe9',
  '--demo-text-muted: #8a8296',
  '--demo-danger: #c2413a',
  '--demo-warn-bg: #fbe9d9',
].join('; ');

const tokenPalette = {
  HeaderBg: 'var(--demo-surface)',
  GroupRow: 'var(--demo-surface-hover)',
  TierMid: 'var(--demo-accent)',
  Grid: 'var(--demo-border)',
  GridLabel: 'var(--demo-text-muted)',
};

export const PaletteFromTokens: Story = {
  args: { ...ExternalRows.args, palette: tokenPalette },
  render: (args) => ({
    components: { GanttChart },
    setup() {
      return { args, tokenStyle, onTaskClick: action('taskClick') };
    },
    template:
      '<div :style="tokenStyle"><GanttChart v-bind="args" @task-click="onTaskClick" /></div>',
  }),
};

export const PaletteFromTokensCanvas: Story = {
  args: { ...PaletteFromTokens.args, backend: 'canvas' },
  render: PaletteFromTokens.render,
};

// The day scale a caller draws beside its own list: every day's number and
// weekday (in the caller's words), weekend bands, a wider day, and a header
// tall enough for the month, the number and the weekday.
export const DayScale: Story = {
  args: {
    ...PaletteFromTokens.args,
    scale: 'day',
    pxPerDay: 32,
    headerHeight: 48,
    weekendDays: [5, 6],
    weekdayLabels: ['月', '火', '水', '木', '金', '土', '日'],
    palette: { ...tokenPalette, Weekend: 'var(--demo-surface-hover)' },
  },
  render: PaletteFromTokens.render,
};

export const DayScaleCanvas: Story = {
  args: { ...DayScale.args, backend: 'canvas' },
  render: PaletteFromTokens.render,
};

// The marks a caller's rows carry: a hard deadline's diamond, a bar outlined in
// its state, a note past a bar whose target runs past its deadline, and a task
// with only a deadline drawn as a dot with a line to its diamond.
const markedRows = [
  { kind: 'group' as const },
  { kind: 'task' as const, task_id: 't1', state: 'done' as const },
  { kind: 'task' as const, task_id: 't2', state: 'normal' as const, hard_deadline: '2026-06-09' },
  { kind: 'group' as const },
  {
    kind: 'task' as const,
    task_id: 't3',
    state: 'warn' as const,
    hard_deadline: '2026-06-08',
    note: '目安 6/10 が厳守 6/8 を越えた',
  },
  {
    kind: 'task' as const,
    task_id: 't4',
    shape: 'point' as const,
    state: 'normal' as const,
    hard_deadline: '2026-06-14',
  },
];

export const BarMarks: Story = {
  args: {
    ...DayScale.args,
    rows: markedRows,
    palette: {
      ...tokenPalette,
      Weekend: 'var(--demo-surface-hover)',
      BarStroke: 'var(--demo-accent)',
      HardDeadline: 'var(--demo-danger)',
      WarnBg: 'var(--demo-warn-bg)',
      WarnStroke: 'var(--demo-danger)',
    },
  },
  render: PaletteFromTokens.render,
};

export const BarMarksCanvas: Story = {
  args: { ...BarMarks.args, backend: 'canvas' },
  render: PaletteFromTokens.render,
};

export const MonthScale: Story = {
  args: { ...vrtArgs, tasks, deps, today: '2026-06-06', scale: 'month', pxPerDay: 8 },
  render: Default.render,
};
