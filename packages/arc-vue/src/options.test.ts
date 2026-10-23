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
});
