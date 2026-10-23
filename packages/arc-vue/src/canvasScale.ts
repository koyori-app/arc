import { RUST_CANVAS_LIMITS } from './wasmContract';

/**
 * canvas の裏打ちを CSS の寸法の何倍にするかを決める。
 *
 * HiDPI の画面では、裏打ちが CSS の寸法のままだと 1 画素が引き伸ばされ、字と線が
 * ぼやける。そこで devicePixelRatio 倍にする。ただし裏打ちの一辺と面積は、core が
 * CSS の寸法に課すのと同じ上限（`RUST_CANVAS_LIMITS`）で頭打ちにする。core が
 * 受けた寸法を、裏打ちで上限の外へ広げないためである。頭打ちになった時は、
 * その分だけ鮮明さを諦め、描けないよりは描ける方を取る。
 */
export function backingScale(
  cssWidth: number,
  cssHeight: number,
  devicePixelRatio: number,
): number {
  if (!(cssWidth > 0) || !(cssHeight > 0)) return 1;
  if (!(devicePixelRatio > 0) || !Number.isFinite(devicePixelRatio)) return 1;
  const { MAX_CANVAS_SIDE_PX, MAX_CANVAS_AREA_PX } = RUST_CANVAS_LIMITS;
  const limit = Math.min(
    MAX_CANVAS_SIDE_PX / cssWidth,
    MAX_CANVAS_SIDE_PX / cssHeight,
    Math.sqrt(MAX_CANVAS_AREA_PX / (cssWidth * cssHeight)),
  );
  return Math.min(devicePixelRatio, limit);
}

/** 裏打ちの寸法。端数は切り捨て、上限を一画素でも越えないようにする。 */
export function backingSize(
  cssWidth: number,
  cssHeight: number,
  devicePixelRatio: number,
): { width: number; height: number } {
  const scale = backingScale(cssWidth, cssHeight, devicePixelRatio);
  return {
    width: Math.floor(cssWidth * scale),
    height: Math.floor(cssHeight * scale),
  };
}

/**
 * 押した点を、描画の命令と当たり判定が使う CSS の座標へ戻す。
 *
 * 裏打ちの寸法ではなく CSS の寸法を基にする。裏打ちを倍にしても、当たり判定の
 * 領域は CSS の座標のままだからである。表示の寸法（`rect`）との比を掛けるのは、
 * 外から CSS で拡大縮小された時にも合わせるためである。
 */
export function cssPointFromClient(
  clientX: number,
  clientY: number,
  rect: { left: number; top: number; width: number; height: number },
  cssWidth: number,
  cssHeight: number,
): { x: number; y: number } | null {
  if (!(rect.width > 0) || !(rect.height > 0)) return null;
  return {
    x: (clientX - rect.left) * (cssWidth / rect.width),
    y: (clientY - rect.top) * (cssHeight / rect.height),
  };
}
