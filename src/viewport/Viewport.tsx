import { useEffect, useImperativeHandle, useRef, useState, type PointerEvent, type Ref } from "react";
import type { Batch } from "./batch";
import { fitBounds, pan, screenToWorld, zoomAt, type Camera, type Point } from "./camera";
import { createRenderer, type Renderer, type Theme } from "./gl";

export type Tool = "select" | "distance" | "area" | "count" | "zoomWindow";

export type ViewportHandle = { fitExtents(): void };

type Props = {
  batch: Batch | null;
  highlight: Set<bigint>;
  tool: Tool;
  onClick(world: Point, screen: Point): void;
  onMove(world: Point): void;
  ref?: Ref<ViewportHandle>;
};

const WHEEL_FACTOR = 1.1;
const FIT_MARGIN_PX = 20;
const DOUBLE_CLICK_MS = 400;
/** Movement below this many pixels is still a click, not a drag. */
const DRAG_PX = 4;

const DARK_QUERY = "(prefers-color-scheme: dark)";

type Drag = { kind: "pan" | "zoomWindow"; start: Point; last: Point };

/** The drawing canvas: wheel zoom at the cursor, middle or space drag to pan, double middle click
 *  for extents, and a drag rectangle to zoom into when the tool is `zoomWindow`. */
export function Viewport({ batch, highlight, tool, onClick, onMove, ref }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<Renderer | null>(null);
  const camera = useRef<Camera>({ cx: 0, cy: 0, scale: 1, width: 1, height: 1 });
  const theme = useRef<Theme>(matchMedia(DARK_QUERY).matches ? "dark" : "light");
  const batchRef = useRef(batch);
  const frame = useRef(0);
  const drag = useRef<Drag | null>(null);
  const space = useRef(false);
  const lastMiddle = useRef(0);
  const [zoomBox, setZoomBox] = useState<[Point, Point] | null>(null);

  function requestDraw() {
    if (frame.current) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = 0;
      renderer.current?.draw(camera.current, theme.current);
    });
  }

  function setCamera(c: Camera) {
    camera.current = c;
    requestDraw();
  }

  function fitExtents() {
    const b = batchRef.current;
    if (b) setCamera(fitBounds(camera.current, b.bounds, FIT_MARGIN_PX));
  }

  useImperativeHandle(ref, () => ({ fitExtents }), []);

  useEffect(() => {
    const canvas = canvasRef.current!;
    renderer.current = createRenderer(canvas);

    const resize = () => {
      const { width, height } = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio;
      canvas.width = Math.max(1, Math.round(width * dpr));
      canvas.height = Math.max(1, Math.round(height * dpr));
      // A drawing fitted while the canvas had no size (hidden at load) is fitted again once it has.
      const hadNoSize = camera.current.width <= 1 || camera.current.height <= 1;
      setCamera({ ...camera.current, width: Math.max(1, width), height: Math.max(1, height) });
      if (hadNoSize) fitExtents();
    };
    // Measure now, so a batch arriving on the first render fits the real size.
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const local = (e: MouseEvent): Point => {
      const r = canvas.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (e.deltaY === 0) return;
      setCamera(zoomAt(camera.current, local(e), e.deltaY > 0 ? WHEEL_FACTOR : 1 / WHEEL_FACTOR));
    };
    // React's wheel listener is passive, so it can't stop the page from scrolling.
    canvas.addEventListener("wheel", onWheel, { passive: false });

    const typing = (e: KeyboardEvent) =>
      e.target instanceof HTMLElement &&
      (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Space" && !typing(e)) space.current = true;
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space") space.current = false;
    };
    const onBlur = () => (space.current = false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", onBlur);

    const dark = matchMedia(DARK_QUERY);
    const onTheme = () => {
      theme.current = dark.matches ? "dark" : "light";
      requestDraw();
    };
    dark.addEventListener("change", onTheme);

    return () => {
      observer.disconnect();
      canvas.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", onBlur);
      dark.removeEventListener("change", onTheme);
      cancelAnimationFrame(frame.current);
      frame.current = 0;
      renderer.current?.dispose();
      renderer.current = null;
    };
  }, []);

  useEffect(() => {
    batchRef.current = batch;
    renderer.current!.setBatch(batch);
    fitExtents();
    requestDraw();
  }, [batch]);

  useEffect(() => {
    renderer.current!.setHighlight(highlight);
    requestDraw();
  }, [highlight]);

  const local = (e: PointerEvent): Point => {
    const r = e.currentTarget.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  function startDrag(e: PointerEvent<HTMLCanvasElement>, kind: Drag["kind"], p: Point) {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { kind, start: p, last: p };
  }

  function onPointerDown(e: PointerEvent<HTMLCanvasElement>) {
    const p = local(e);
    if (e.button === 1) {
      e.preventDefault(); // no middle-click autoscroll or paste
      const now = performance.now();
      if (now - lastMiddle.current < DOUBLE_CLICK_MS) {
        lastMiddle.current = 0;
        fitExtents();
        return;
      }
      lastMiddle.current = now;
      startDrag(e, "pan", p);
    } else if (e.button === 0 && space.current) {
      startDrag(e, "pan", p);
    } else if (e.button === 0 && tool === "zoomWindow") {
      startDrag(e, "zoomWindow", p);
      setZoomBox([p, p]);
    } else if (e.button === 0) {
      onClick(screenToWorld(camera.current, p), p);
    }
  }

  function onPointerMove(e: PointerEvent<HTMLCanvasElement>) {
    const p = local(e);
    const d = drag.current;
    if (d?.kind === "pan") {
      setCamera(pan(camera.current, p[0] - d.last[0], p[1] - d.last[1]));
    } else if (d?.kind === "zoomWindow") {
      setZoomBox([d.start, p]);
    }
    if (d) d.last = p;
    onMove(screenToWorld(camera.current, p));
  }

  function onPointerUp() {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    const [dx, dy] = [Math.abs(d.last[0] - d.start[0]), Math.abs(d.last[1] - d.start[1])];
    // A middle drag is a pan, not the first half of a double click.
    if (d.kind === "pan" && Math.max(dx, dy) > DRAG_PX) lastMiddle.current = 0;
    if (d.kind === "zoomWindow") {
      setZoomBox(null);
      if (dx > DRAG_PX && dy > DRAG_PX) {
        const [ax, ay] = screenToWorld(camera.current, d.start);
        const [bx, by] = screenToWorld(camera.current, d.last);
        const bounds: [Point, Point] = [
          [Math.min(ax, bx), Math.min(ay, by)],
          [Math.max(ax, bx), Math.max(ay, by)],
        ];
        setCamera(fitBounds(camera.current, bounds, 0));
      }
    }
  }

  return (
    <div style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden" }}>
      <canvas
        ref={canvasRef}
        role="img"
        aria-label="Drawing"
        style={{ display: "block", width: "100%", height: "100%", touchAction: "none" }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          drag.current = null;
          setZoomBox(null);
        }}
      />
      {zoomBox && (
        <div
          style={{
            position: "absolute",
            left: Math.min(zoomBox[0][0], zoomBox[1][0]),
            top: Math.min(zoomBox[0][1], zoomBox[1][1]),
            width: Math.abs(zoomBox[1][0] - zoomBox[0][0]),
            height: Math.abs(zoomBox[1][1] - zoomBox[0][1]),
            border: "1px dashed #6E4A9E",
            pointerEvents: "none",
          }}
        />
      )}
    </div>
  );
}
