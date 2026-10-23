/**
 * The `options_json` string the Wasm entry points take.
 *
 * `today`, the scroll window and the time zone used to be three positional
 * arguments; they are keys of one JSON object now, so a new option adds a key
 * and not a position. Only the keys that have a value are written, and when
 * none has one the result is `undefined`, which the entry points read as the
 * defaults.
 */
export interface ScrollViewportOption {
  scroll_y: number;
  client_height: number;
}

export interface ChartOptionsInput {
  today?: string | null;
  scrollViewport?: ScrollViewportOption | null;
  tzOffsetMinutes?: number | null;
}

export function buildOptionsJson(input: ChartOptionsInput): string | undefined {
  const options: Record<string, unknown> = {};
  if (input.today != null) options.today = input.today;
  if (input.scrollViewport != null) options.scroll_viewport = input.scrollViewport;
  if (input.tzOffsetMinutes != null) {
    // JSON cannot carry NaN or Infinity (`JSON.stringify` would turn them into
    // `null`, which means "no offset"). A string keeps the unusable value
    // visible, so the entry point refuses it as it refuses any bad offset.
    options.tz_offset_minutes = Number.isFinite(input.tzOffsetMinutes)
      ? input.tzOffsetMinutes
      : String(input.tzOffsetMinutes);
  }
  return Object.keys(options).length === 0 ? undefined : JSON.stringify(options);
}
