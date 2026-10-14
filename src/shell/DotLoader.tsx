import type { CSSProperties } from "react";

/** The motions: a sweep across the grid, a spiral in, a pulse out from the centre, a chase round
 *  the border. All run on one CSS keyframe; a dot's place in the motion is its `--p` (0 to under 1). */
export type DotPattern = "scan" | "spiral" | "pulse" | "chase";
/** `sm` is a 3 by 3 grid (about 12 px, one status-bar line), `md` a 5 by 5 grid (about 26 px). */
export type DotSize = "sm" | "md";

type Props = {
  pattern?: DotPattern;
  size?: DotSize;
  /** What is being waited for. Without it the loader is decoration (`aria-hidden`) for a text
   *  beside it that already says it. With it, an image-like element with that name: it is not a
   *  live region, so nothing is announced again and again while the dots move. */
  label?: string;
  className?: string;
};

type Dot = {
  /** Place in the motion, 0 up to (not including) 1; null for a dot the pattern leaves resting. */
  p: number | null;
};

const SIDE: Record<DotSize, number> = { sm: 3, md: 5 };

/** Row and column of the cells along the border, clockwise from the top start corner. */
function border(n: number): [number, number][] {
  const cells: [number, number][] = [];
  for (let c = 0; c < n; c++) cells.push([0, c]);
  for (let r = 1; r < n; r++) cells.push([r, n - 1]);
  for (let c = n - 2; c >= 0; c--) cells.push([n - 1, c]);
  for (let r = n - 2; r >= 1; r--) cells.push([r, 0]);
  return cells;
}

/** All cells in a clockwise spiral that ends in the middle. */
function spiral(n: number): [number, number][] {
  const cells: [number, number][] = [];
  let top = 0;
  let left = 0;
  let bottom = n - 1;
  let right = n - 1;
  while (top <= bottom && left <= right) {
    for (let c = left; c <= right; c++) cells.push([top, c]);
    for (let r = top + 1; r <= bottom; r++) cells.push([r, right]);
    if (top < bottom) for (let c = right - 1; c >= left; c--) cells.push([bottom, c]);
    if (left < right) for (let r = bottom - 1; r > top; r--) cells.push([r, left]);
    top++;
    left++;
    bottom--;
    right--;
  }
  return cells;
}

/** The dots of a grid, row by row, each with its place in the pattern's motion. Pure: the same
 *  pattern and size give the same dots. */
export function dotsFor(pattern: DotPattern, size: DotSize): Dot[] {
  const n = SIDE[size];
  const mid = (n - 1) / 2;
  const out: Dot[] = Array.from({ length: n * n }, () => ({ p: null }));
  const place = (r: number, c: number, p: number) => {
    out[r * n + c] = { p };
  };
  if (pattern === "scan") {
    // Column by column from the start side (the right side in Arabic).
    for (let r = 0; r < n; r++) for (let c = 0; c < n; c++) place(r, c, c / n);
  } else if (pattern === "pulse") {
    // Ring by ring from the centre; the same from every side.
    for (let r = 0; r < n; r++)
      for (let c = 0; c < n; c++) place(r, c, Math.max(Math.abs(r - mid), Math.abs(c - mid)) / (mid + 1));
  } else {
    const cells = pattern === "spiral" ? spiral(n) : border(n);
    cells.forEach(([r, c], i) => place(r, c, i / cells.length));
  }
  return out;
}

/** A small grid of dots that move in a quiet pattern while something waits. Pure CSS motion
 *  (`.dot-loader` in `theme.css`): no timer. The grid follows the text direction, so a sweep or a
 *  spiral starts on the start side and turns the other way in Arabic. With reduced motion the dots
 *  stand still, bright where the motion starts and fading along it. */
export function DotLoader({ pattern = "scan", size = "sm", label, className }: Props) {
  const dots = dotsFor(pattern, size);
  return (
    <span
      className={`dot-loader dot-${size} dot-${pattern}${className ? ` ${className}` : ""}`}
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    >
      {dots.map((dot, i) => (
        <i
          key={i}
          className={dot.p === null ? "rest" : undefined}
          style={dot.p === null ? undefined : ({ "--p": dot.p.toFixed(3) } as CSSProperties)}
        />
      ))}
    </span>
  );
}
