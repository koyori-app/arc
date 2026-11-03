import { describe, expect, it } from 'vitest';
import { buildOptionsJson } from './options';

describe('buildOptionsJson with lanes', () => {
  const lanes = [
    { id: 'alice', label: 'alice' },
    { id: 'none', label: '担当なし' },
  ];

  it('writes `lanes` as given, in the order given', () => {
    expect(JSON.parse(buildOptionsJson({ lanes })!)).toEqual({ lanes });
  });

  it('writes an empty list: a chart in lanes with none is a choice, not an absence', () => {
    expect(buildOptionsJson({ lanes: [] })).toBe('{"lanes":[]}');
  });

  it('leaves `lanes` out when it was not given, so the string is the one written before it existed', () => {
    expect(buildOptionsJson({ lanes: null })).toBeUndefined();
    expect(buildOptionsJson({ lanes: undefined })).toBeUndefined();
    expect(buildOptionsJson({ today: '2026-06-03', tzOffsetMinutes: 540, lanes: undefined }))
      .toBe('{"today":"2026-06-03","tz_offset_minutes":540}');
    expect(Object.keys(JSON.parse(buildOptionsJson({ today: '2026-06-03' })!))).not.toContain('lanes');
  });

  it('sits beside the other keys without changing them', () => {
    expect(
      JSON.parse(
        buildOptionsJson({
          today: '2026-06-03',
          scrollViewport: { scroll_y: 40, client_height: 600 },
          legend: 'hidden',
          lanes,
        })!,
      ),
    ).toEqual({
      today: '2026-06-03',
      scroll_viewport: { scroll_y: 40, client_height: 600 },
      legend: 'hidden',
      lanes,
    });
  });
});
