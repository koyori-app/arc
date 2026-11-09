import { describe, expect, it } from 'vitest';
import { buildOptionsJson } from './options';

describe('buildOptionsJson', () => {
  it('is undefined when nothing was chosen, so the entry point draws the defaults', () => {
    expect(buildOptionsJson({})).toBeUndefined();
    expect(buildOptionsJson({ today: null, scrollViewport: null, tzOffsetMinutes: null })).toBeUndefined();
    expect(buildOptionsJson({ today: undefined })).toBeUndefined();
  });

  it('writes only the keys that have a value', () => {
    expect(JSON.parse(buildOptionsJson({ today: '2026-06-16' })!)).toEqual({ today: '2026-06-16' });
    expect(JSON.parse(buildOptionsJson({ tzOffsetMinutes: 0 })!)).toEqual({ tz_offset_minutes: 0 });
    expect(
      JSON.parse(
        buildOptionsJson({
          today: '2026-06-16T09:00:00+09:00',
          scrollViewport: { scroll_y: 40, client_height: 600 },
          tzOffsetMinutes: 540,
        })!,
      ),
    ).toEqual({
      today: '2026-06-16T09:00:00+09:00',
      scroll_viewport: { scroll_y: 40, client_height: 600 },
      tz_offset_minutes: 540,
    });
  });

  it('keeps an empty `today` (a blank one means no marker, and the entry point decides)', () => {
    expect(JSON.parse(buildOptionsJson({ today: '' })!)).toEqual({ today: '' });
  });

  it('never writes a `legend` key while the caller did not ask for one', () => {
    const json = buildOptionsJson({ today: '2026-06-16', tzOffsetMinutes: 540 })!;
    expect(Object.keys(JSON.parse(json))).not.toContain('legend');
  });

  it('keeps an unusable offset visible instead of letting JSON turn it into null', () => {
    // JSON.stringify(NaN) is "null", which the entry point reads as "no offset".
    expect(JSON.parse(buildOptionsJson({ tzOffsetMinutes: Number.NaN })!)).toEqual({
      tz_offset_minutes: 'NaN',
    });
    expect(JSON.parse(buildOptionsJson({ tzOffsetMinutes: Number.POSITIVE_INFINITY })!)).toEqual({
      tz_offset_minutes: 'Infinity',
    });
  });

  it('writes `legend` only when it was given, and leaves the other keys as they were', () => {
    expect(JSON.parse(buildOptionsJson({ legend: 'hidden' })!)).toEqual({ legend: 'hidden' });
    expect(JSON.parse(buildOptionsJson({ legend: 'shown' })!)).toEqual({ legend: 'shown' });
    expect(buildOptionsJson({ legend: null })).toBeUndefined();
    expect(buildOptionsJson({ legend: undefined })).toBeUndefined();
    expect(JSON.parse(buildOptionsJson({ today: '2026-06-16', legend: 'hidden' })!)).toEqual({
      today: '2026-06-16',
      legend: 'hidden',
    });
  });

  it('is byte for byte what it was before `legend` existed, for the same inputs', () => {
    expect(buildOptionsJson({ today: '2026-06-03', tzOffsetMinutes: 540 }))
      .toBe('{"today":"2026-06-03","tz_offset_minutes":540}');
    expect(buildOptionsJson({ scrollViewport: { scroll_y: 0, client_height: 600 } }))
      .toBe('{"scroll_viewport":{"scroll_y":0,"client_height":600}}');
  });
  it('writes `row_labels` and `percent_labels` only when they were given', () => {
    expect(JSON.parse(buildOptionsJson({ rowLabels: 'hidden' })!)).toEqual({ row_labels: 'hidden' });
    expect(JSON.parse(buildOptionsJson({ percentLabels: 'hidden' })!)).toEqual({
      percent_labels: 'hidden',
    });
    expect(buildOptionsJson({ rowLabels: null, percentLabels: undefined })).toBeUndefined();
    expect(
      JSON.parse(
        buildOptionsJson({
          today: '2026-06-16',
          legend: 'hidden',
          rowLabels: 'hidden',
          percentLabels: 'shown',
        })!,
      ),
    ).toEqual({
      today: '2026-06-16',
      legend: 'hidden',
      row_labels: 'hidden',
      percent_labels: 'shown',
    });
    const json = buildOptionsJson({ today: '2026-06-16', legend: 'hidden' })!;
    expect(Object.keys(JSON.parse(json))).not.toContain('row_labels');
    expect(Object.keys(JSON.parse(json))).not.toContain('percent_labels');
  });
  it('writes the size keys and `bar_align` only when they were given', () => {
    expect(
      JSON.parse(
        buildOptionsJson({
          rowHeightPx: 32,
          headerHeightPx: 48,
          barHeightPx: 12,
          barAlign: 'bottom',
        })!,
      ),
    ).toEqual({ row_height_px: 32, header_height_px: 48, bar_height_px: 12, bar_align: 'bottom' });
    expect(
      buildOptionsJson({ rowHeightPx: null, headerHeightPx: undefined, barAlign: null }),
    ).toBeUndefined();
  });
  it('writes the scale keys only when they were given', () => {
    expect(
      JSON.parse(
        buildOptionsJson({
          pxPerDay: 32,
          scale: 'day',
          weekendDays: [5, 6],
          weekdayLabels: ['月', '火', '水', '木', '金', '土', '日'],
        })!,
      ),
    ).toEqual({
      px_per_day: 32,
      scale: 'day',
      weekend_days: [5, 6],
      weekday_labels: ['月', '火', '水', '木', '金', '土', '日'],
    });
    expect(
      buildOptionsJson({ pxPerDay: null, scale: undefined, weekendDays: null, weekdayLabels: null }),
    ).toBeUndefined();
  });
  it('writes the view only when it was given, and only its two days', () => {
    const view = { from: '2026-11-09', to: '2026-11-15', label: 'x' } as {
      from: string;
      to: string;
    };
    expect(JSON.parse(buildOptionsJson({ view })!)).toEqual({
      view: { from: '2026-11-09', to: '2026-11-15' },
    });
    expect(buildOptionsJson({ view: null })).toBeUndefined();
  });
});
