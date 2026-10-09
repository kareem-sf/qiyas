import { useEffect, useImperativeHandle, useRef, useState, type PointerEvent, type Ref } from "react";
import {
  glyphsNeedRedraw,
  picturesToFetch,
  picturesToRelease,
  type AtlasPage,
  type Batch,
  type HeldPicture,
  type Target,
} from "./batch";
import {
  fitBounds,
  pan,
  resized,
  screenToWorld,
  visibleBounds,
  zoomAt,
  type Bounds,
  type Camera,
  type Point,
} from "./camera";
import { createRenderer, type Renderer, type Theme } from "./gl";

export type Tool = "select" | "distance" | "area" | "count" | "zoomWindow";

export type ViewportHandle = {
  fitExtents(): void;
  fitTo(bounds: Bounds): void;
  /** Screen (device) pixels per drawing unit at the current zoom. */
  pxPerUnit(): number;
  /** The drawing area in view. */
  visible(): Bounds;
};

type Props = {
  batch: Batch | null;
  /** Every glyph atlas page received so far; the batch's quads draw from them. */
  atlas?: AtlasPage[];
  highlight: Target[];
  tool: Tool;
  theme: Theme;
  /** Violet segments `x0 y0 x1 y1` in absolute drawing coordinates, drawn over the drawing. */
  overlay?: Float64Array | null;
  /** Drawing units between the grid's dots; null or left out for no grid. */
  grid?: number | null;
  /** A left click; `worldPerPixel` is the zoom at that moment, for pick tolerances. */
  onClick(world: Point, screen: Point, worldPerPixel: number): void;
  /** The cursor moved; `worldPerPixel` is the zoom at that moment, for snap tolerances. */
  onMove(world: Point, worldPerPixel: number): void;
  /** The zoom has more than doubled or halved since the batch's glyphs were drawn: draw them
   *  again for `pxPerUnit` screen pixels per drawing unit. Called once the zoom settles. */
  onZoomHint?(pxPerUnit: number): void;
  /** The underlay pictures held, by image id. */
  pictures?: Map<number, HeldPicture>;
  /** Some underlay picture in view is missing or was drawn for a zoom too far off, or one held is
   *  out of view: fetch and let go for `pxPerUnit` and the area `visible`. Called once the view
   *  settles. */
  onPictureHint?(pxPerUnit: number, visible: Bounds): void;
  ref?: Ref<ViewportHandle>;
};

const WHEEL_FACTOR = 1.1;
const FIT_MARGIN_PX = 20;
const DOUBLE_CLICK_MS = 400;
/** Movement below this many pixels is still a click, not a drag. */
const DRAG_PX = 4;
/** How long the zoom must rest before glyphs are asked for at the new size. */
const ZOOM_SETTLE_MS = 200;
const NO_PAGES: AtlasPage[] = [];
const NO_PICTURES = new Map<number, HeldPicture>();

type Drag = { kind: "pan" | "zoomWindow"; start: Point; last: Point };

/** The drawing canvas: wheel zoom at the cursor, middle or space drag to pan, double middle click
 *  for extents, and a drag rectangle to zoom into when the tool is `zoomWindow`. */
export function Viewport({
  batch,
  atlas = NO_PAGES,
  highlight,
  tool,
  theme,
  overlay = null,
  grid = null,
  onClick,
  onMove,
  onZoomHint,
  pictures = NO_PICTURES,
  onPictureHint,
  ref,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<Renderer | null>(null);
  const camera = useRef<Camera>({ cx: 0, cy: 0, scale: 1, width: 1, height: 1 });
  /** The world box the user (or the app) last chose to show; a resize keeps it in view. */
  const view = useRef<Bounds | null>(null);
  const themeRef = useRef(theme);
  const batchRef = useRef(batch);
  const frame = useRef(0);
  const drag = useRef<Drag | null>(null);
  const space = useRef(false);
  const lastMiddle = useRef(0);
  const [zoomBox, setZoomBox] = useState<[Point, Point] | null>(null);
  const zoomTimer = useRef(0);
  const onZoomHintRef = useRef(onZoomHint);
  onZoomHintRef.current = onZoomHint;
  const picturesRef = useRef(pictures);
  picturesRef.current = pictures;
  const pictureTimer = useRef(0);
  const onPictureHintRef = useRef(onPictureHint);
  onPictureHintRef.current = onPictureHint;

  const pxPerUnit = () => window.devicePixelRatio / camera.current.scale;

  /** Asks for glyphs at the current size once the zoom has rested, if they are off by over 2x. */
  function checkGlyphs() {
    const stale = () => {
      const b = batchRef.current;
      return !!b && b.quads.length > 0 && glyphsNeedRedraw(b.pxPerUnit, pxPerUnit());
    };
    if (!stale()) return;
    clearTimeout(zoomTimer.current);
    zoomTimer.current = window.setTimeout(() => {
      if (stale()) onZoomHintRef.current?.(pxPerUnit());
    }, ZOOM_SETTLE_MS);
  }

  /** Asks for underlay pictures once the view has rested, when some in view are missing or were
   *  drawn for a zoom over 2x off, or some held are out of view. */
  function checkPictures() {
    const stale = () => {
      const [b, held, visible] = [batchRef.current, picturesRef.current, visibleBounds(camera.current)];
      return (
        picturesToFetch(b, held, pxPerUnit(), visible).length > 0 ||
        picturesToRelease(b, held, visible).length > 0
      );
    };
    if (!stale()) return;
    clearTimeout(pictureTimer.current);
    pictureTimer.current = window.setTimeout(() => {
      if (stale()) onPictureHintRef.current?.(pxPerUnit(), visibleBounds(camera.current));
    }, ZOOM_SETTLE_MS);
  }

  function requestDraw() {
    if (frame.current) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = 0;
      renderer.current?.draw(camera.current, themeRef.current);
    });
  }

  function setCamera(c: Camera, keepView = false) {
    camera.current = c;
    if (!keepView) view.current = visibleBounds(c);
    requestDraw();
    checkGlyphs();
    checkPictures();
  }

  function fitExtents() {
    const b = batchRef.current;
    if (b) setCamera(fitBounds(camera.current, b.bounds, FIT_MARGIN_PX));
  }

  useImperativeHandle(
    ref,
    () => ({
      fitExtents,
      fitTo: (b) => setCamera(fitBounds(camera.current, b, FIT_MARGIN_PX)),
      pxPerUnit,
      visible: () => visibleBounds(camera.current),
    }),
    [],
  );

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
      const [w, h] = [Math.max(1, width), Math.max(1, height)];
      // A side panel widening or narrowing the canvas keeps what was in view, centred.
      if (hadNoSize || !view.current) setCamera({ ...camera.current, width: w, height: h });
      else setCamera(resized(camera.current, w, h, view.current), true);
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

    return () => {
      observer.disconnect();
      canvas.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", onBlur);
      cancelAnimationFrame(frame.current);
      frame.current = 0;
      clearTimeout(zoomTimer.current);
      clearTimeout(pictureTimer.current);
      renderer.current?.dispose();
      renderer.current = null;
    };
  }, []);

  // Before the batch, so its quads find their pages.
  useEffect(() => {
    renderer.current!.setAtlas(atlas);
    requestDraw();
  }, [atlas]);

  // No fit here: hiding a layer gives a new batch but should keep the view. The app fits on open.
  useEffect(() => {
    batchRef.current = batch;
    renderer.current!.setBatch(batch);
    requestDraw();
    checkGlyphs();
    checkPictures();
  }, [batch]);

  useEffect(() => {
    renderer.current!.setPictures(pictures);
    requestDraw();
  }, [pictures]);

  useEffect(() => {
    themeRef.current = theme;
    requestDraw();
  }, [theme]);

  useEffect(() => {
    renderer.current!.setHighlight(highlight);
    requestDraw();
  }, [highlight]);

  useEffect(() => {
    renderer.current!.setOverlay(overlay);
    requestDraw();
  }, [overlay]);

  useEffect(() => {
    renderer.current!.setGrid(grid);
    requestDraw();
  }, [grid]);

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
      onClick(screenToWorld(camera.current, p), p, camera.current.scale);
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
    onMove(screenToWorld(camera.current, p), camera.current.scale);
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
