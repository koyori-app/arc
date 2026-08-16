import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  CHART_BOTTOM_PADDING_PX,
  EMPTY_SVG_MARKUP,
  RUST_ERROR_CODES,
  RUST_LAYOUT,
} from './wasmContract';

/**
 * The cross-language contract. Both sides declare the same literals; only this
 * test proves they still agree. It reads the Rust sources rather than the built
 * Wasm so it runs without a wasm-pack build — and so editing a Rust literal
 * fails here immediately.
 */

function readCrateSource(relativePath: string): string {
  const path = fileURLToPath(
    new URL(`../../../crates/koyori-arc-core/src/${relativePath}`, import.meta.url),
  );
  return readFileSync(path, 'utf8');
}

function rustErrorCodes(): Record<string, string> {
  const source = readCrateSource('error.rs');
  const codes: Record<string, string> = {};
  for (const [, name, value] of source.matchAll(
    /pub const (CODE_[A-Z_]+): &str = "([^"]*)";/g,
  )) {
    codes[name] = value;
  }
  return codes;
}

function rustEmptySvg(): string {
  const source = readCrateSource('backend/svg.rs');
  const match = source.match(
    /pub fn empty_svg\(\) -> String \{\s*r#"([\s\S]*?)"#\s*\.to_string\(\)/,
  );
  expect(match, 'empty_svg() literal found in backend/svg.rs').not.toBeNull();
  return match![1];
}

function rustLayoutConstants(): Record<string, number> {
  const source = readCrateSource('display_list/constants.rs');
  const values: Record<string, number> = {};
  for (const [, name, value] of source.matchAll(
    /pub const ([A-Z_]+): f64 = ([0-9]+(?:\.[0-9]+)?);/g,
  )) {
    values[name] = Number(value);
  }
  return values;
}

function rustChartBottomPadding(): number {
  const source = readCrateSource('render.rs');
  const match = source.match(
    /const CHART_BOTTOM_PADDING_PX: f64 = ([0-9]+(?:\.[0-9]+)?);/,
  );
  expect(match, 'CHART_BOTTOM_PADDING_PX literal found in render.rs').not.toBeNull();
  return Number(match![1]);
}

describe('koyori-arc-core contract', () => {
  it('mirrors every error code, by name and by value', () => {
    const fromRust = rustErrorCodes();
    expect(Object.keys(fromRust).length).toBeGreaterThan(0);
    expect(fromRust).toEqual(RUST_ERROR_CODES);
  });

  it('mirrors the empty-chart markup byte for byte', () => {
    expect(rustEmptySvg()).toBe(EMPTY_SVG_MARKUP);
  });

  it('mirrors every layout constant the chart height is built from', () => {
    const fromRust = rustLayoutConstants();
    expect(Object.keys(fromRust).length).toBeGreaterThan(0);
    for (const [name, pinned] of Object.entries(RUST_LAYOUT)) {
      expect(fromRust, `${name} found in display_list/constants.rs`)
        .toHaveProperty(name);
      expect(fromRust[name], name).toBe(pinned);
    }
  });

  it('mirrors the chart bottom padding', () => {
    expect(rustChartBottomPadding()).toBe(CHART_BOTTOM_PADDING_PX);
  });
});
