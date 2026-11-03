# @koyori-app/arc-vue

Vue 3 Gantt chart component for [arc](https://github.com/koyori-app/arc). The layout is computed by the `@koyori-app/arc` WebAssembly core; this package draws it.

Pre-release (`0.1.0-alpha`). The API may change between versions.

## Install

Install both packages. `@koyori-app/arc` (the Wasm core) and `vue` are peer dependencies.

```sh
npm install @koyori-app/arc-vue @koyori-app/arc
```

## Usage

Import the stylesheet once, then pass tasks and dependencies to `GanttChart`.

```vue
<script setup lang="ts">
import { GanttChart, type GanttDep, type GanttTask } from '@koyori-app/arc-vue';
import '@koyori-app/arc-vue/style.css';

const tasks: GanttTask[] = [
  { id: 'a', title: 'Design', progress_pct: 100, start: '2026-06-01', end: '2026-06-05' },
  { id: 'b', title: 'Build', progress_pct: 40, start: '2026-06-08', end: '2026-06-19' },
];
const deps: GanttDep[] = [{ blocker_task_id: 'a', blocked_task_id: 'b' }];
</script>

<template>
  <GanttChart :tasks="tasks" :deps="deps" today="2026-06-10" @task-click="(task) => console.log(task.id)" />
</template>
```

### Data shapes

```ts
interface GanttTask {
  id: string;
  title: string;
  progress_pct: number; // 0 to 100
  start: string; // ISO 8601 date ("2026-06-01") or date-time
  end?: string; // same formats as start
  lane_ids?: string[]; // lanes this task belongs to (only read when `lanes` is given)
}

interface GanttLane {
  id: string;
  label: string;
}

interface GanttDep {
  blocker_task_id: string;
  blocked_task_id: string;
}
```

### Props

| Prop | Type | Notes |
| --- | --- | --- |
| `tasks` | `GanttTask[]` | Required. |
| `deps` | `GanttDep[]` | Dependency arrows. |
| `today` | `string` | ISO 8601 date or date-time for the today line. |
| `tzOffsetMinutes` | `number` | Viewer time zone as minutes east of UTC (JST is `540`). Date-times with an offset are drawn on this zone's calendar day. Never read from the browser. |
| `legend` | `'shown' \| 'hidden'` | `'hidden'` leaves the colour legend out (see `loadLegendSpec`). |
| `lanes` | `GanttLane[]` | Draws the chart in lanes (see below). Left out, the chart is the usual one row per task. |
| `backend` | `'svg' \| 'canvas'` | Defaults to `svg`. |
| `deviceTier` | `'low' \| 'high'` | Overrides the detected device tier (useful for visual tests). |

The `taskClick` event receives the clicked `GanttTask`.

### Lanes

Give `lanes` to draw one lane per entry, top first, with the name in the left column, instead of one row per task.
Each task goes into every lane its `lane_ids` names; the bars of a lane are packed into as many rows as they need, so bars that overlap in time sit one above the other.
A typical use is one lane per assignee.

```vue
<GanttChart
  :lanes="[{ id: 'alice', label: 'alice' }, { id: 'none', label: 'Unassigned' }]"
  :tasks="[
    { id: 't1', title: 'Design', progress_pct: 40, start: '2026-06-01', end: '2026-06-05', lane_ids: ['alice'] },
    { id: 't2', title: 'Pairing', progress_pct: 0, start: '2026-06-03', end: '2026-06-08', lane_ids: ['alice', 'none'] },
  ]"
/>
```

- A task in two lanes is drawn once in each, and clicking either bar emits the same task.
- A task in none of the lanes is not drawn, and a lane id no lane carries is ignored. Put tasks nobody owns in a lane of their own.
- A lane nobody belongs to keeps one row and its name. An empty `lanes` array is a chart in lanes with no lanes.
- Dependency arrows and the progress status line are not drawn (a task in two lanes has no single place for an arrow to start or end), and the legend leaves out the progress-line sample. `deps` is accepted and ignored.
- The chart is as tall as its rows pack, which only the Wasm knows, so the height is read off the drawing (until then it is a guess of one row per lane).
- Without `lanes` nothing changes, and `lane_ids` are not read.

## Server-side rendering

The component loads the Wasm module in `onMounted`, so it renders on the server without touching Wasm and draws the chart after it mounts in the browser. Do not call the Wasm exports of `@koyori-app/arc` (or `loadLegendSpec`, which initialises the module) during server rendering; call them from `onMounted` or other client-only code.

## License

MPL-2.0 OR GPL-2.0-or-later.
