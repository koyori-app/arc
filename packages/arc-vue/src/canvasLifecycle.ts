type ResettableCanvas = Pick<HTMLCanvasElement, 'width' | 'height' | 'style' | 'getContext'>;

/** Clear both the drawing buffer and its layout footprint after empty/error output. */
export function resetCanvasElement(canvas: ResettableCanvas | null): void {
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  if (ctx) {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
  }
  canvas.width = 0;
  canvas.height = 0;
  canvas.style.width = '0px';
  canvas.style.height = '0px';
}
