import { execSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkgRoot = join(__dirname, '..');
const corePkg = join(pkgRoot, '../../crates/koyori-arc-core/pkg');
const builtCss = join(pkgRoot, 'dist', 'arc-vue.css');

/**
 * This test inspects build output, so it states that precondition itself rather
 * than relying on the CI step order. Reading dist/ directly would fail with a
 * bare ENOENT for anyone who runs `pnpm test` before `pnpm build` — in a fresh
 * clone, or after the workflow steps are reordered.
 */
function requireBuiltDist(): void {
  if (existsSync(builtCss)) return;
  throw new Error(
    [
      `Missing build output: ${builtCss}`,
      'This test packs the package and resolves @koyori-app/arc-vue/style.css, both of which read dist/.',
      'Run `pnpm build` in packages/arc-vue first, then re-run `pnpm test`.',
    ].join('\n'),
  );
}

describe('consumer pack', () => {
  it('packs dist CSS and resolves @koyori-app/arc-vue/style.css', () => {
    requireBuiltDist();

    const pkg = JSON.parse(readFileSync(join(pkgRoot, 'package.json'), 'utf8')) as {
      exports: Record<string, string | { import?: string }>;
      sideEffects: string[];
    };

    const styleExport = pkg.exports['./style.css'];
    expect(styleExport).toBe('./dist/arc-vue.css');
    expect(pkg.sideEffects).toEqual(['**/*.css']);

    expect(readFileSync(builtCss, 'utf8').length).toBeGreaterThan(0);

    const tmp = mkdtempSync(join(tmpdir(), 'arc-vue-pack-'));
    try {
      const packOutput = execSync(`pnpm pack --pack-destination "${tmp}"`, {
        cwd: pkgRoot,
        encoding: 'utf8',
      });
      const tarball = packOutput.trim().split('\n').at(-1)!;
      expect(tarball).toMatch(/\.tgz$/);

      const installDir = join(tmp, 'install');
      execSync(`mkdir -p "${installDir}"`, { encoding: 'utf8' });
      execSync(
        `npm install "${tarball}" "file:${corePkg}" --legacy-peer-deps --no-save`,
        { cwd: installDir, encoding: 'utf8', stdio: 'pipe' },
      );

      const require = createRequire(join(installDir, 'package.json'));
      const resolved = require.resolve('@koyori-app/arc-vue/style.css');
      expect(readFileSync(resolved, 'utf8')).toContain('.koyori-gantt-scroll');
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
