import { open } from "@tauri-apps/plugin-dialog";
import { useRef, useState } from "react";
import { openDrawing, renderBatch } from "./api";
import { parseBatch, type Batch } from "./viewport/batch";
import { Viewport, type Tool, type ViewportHandle } from "./viewport/Viewport";

const NO_HIGHLIGHT = new Set<bigint>();

// Temporary shell to try the viewport by hand; the real ribbon and panels come in a later task.
export default function App() {
  const [batch, setBatch] = useState<Batch | null>(null);
  const [tool, setTool] = useState<Tool>("select");
  const [error, setError] = useState<string | null>(null);
  const viewport = useRef<ViewportHandle>(null);

  async function openFile() {
    const path = await open({ filters: [{ name: "Drawings", extensions: ["dwg", "dxf"] }] });
    if (typeof path !== "string") return;
    try {
      await openDrawing(path);
      setBatch(parseBatch(await renderBatch()));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div style={{ position: "fixed", inset: 0, display: "flex", flexDirection: "column" }}>
      <div style={{ display: "flex", gap: 8, padding: 8 }}>
        <button onClick={openFile}>Open</button>
        <button onClick={() => viewport.current?.fitExtents()}>Zoom extents</button>
        <button
          aria-pressed={tool === "zoomWindow"}
          onClick={() => setTool(tool === "zoomWindow" ? "select" : "zoomWindow")}
        >
          Zoom window
        </button>
        {error && <span role="alert">{error}</span>}
      </div>
      <div style={{ flex: 1, minHeight: 0 }}>
        <Viewport
          ref={viewport}
          batch={batch}
          highlight={NO_HIGHLIGHT}
          tool={tool}
          onClick={() => {}}
          onMove={() => {}}
        />
      </div>
    </div>
  );
}
