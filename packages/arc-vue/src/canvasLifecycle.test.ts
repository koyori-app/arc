import { describe, expect, it, vi } from 'vitest';
import { resetCanvasElement } from './canvasLifecycle';

function paintedCanvas() {
  const clearRect = vi.fn();
  const canvas = {
    width: 1200,
    height: 800,
    style: { width: '1200px', height: '800px' },
    getContext: vi.fn(() => ({ clearRect })),
  };
  return { canvas, clearRect };
}

describe('resetCanvasElement', () => {
  // One case, stated once: `resetCanvasElement` does not branch on why the
  // canvas is being torn down, so the former it.each ran the same body twice.
  it('removes bitmap and CSS dimensions', () => {
    const { canvas, clearRect } = paintedCanvas();

    resetCanvasElement(canvas as unknown as HTMLCanvasElement);

    expect(clearRect).toHaveBeenCalledWith(0, 0, 1200, 800);
    expect(canvas.width).toBe(0);
    expect(canvas.height).toBe(0);
    expect(canvas.style.width).toBe('0px');
    expect(canvas.style.height).toBe('0px');
  });
});
