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
| `backend` | `'svg' \| 'canvas'` | Defaults to `svg`. |
| `deviceTier` | `'low' \| 'high'` | Overrides the detected device tier (useful for visual tests). |

The `taskClick` event receives the clicked `GanttTask`.

## Server-side rendering

The component loads the Wasm module in `onMounted`, so it renders on the server without touching Wasm and draws the chart after it mounts in the browser. Do not call the Wasm exports of `@koyori-app/arc` (or `loadLegendSpec`, which initialises the module) during server rendering; call them from `onMounted` or other client-only code.

## License

MPL-2.0 OR GPL-2.0-or-later.
