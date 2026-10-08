/**
 * WebGL2 line renderer for a `Batch`.
 *
 * Precision: drawings can sit far from (0, 0) (survey grids in metres or millimetres), where f32
 * keeps only a few useful digits. So no absolute coordinate ever reaches the GPU. The segments arrive
 * relative to `batch.origin` and are uploaded once as f32. On each draw the view transform is built
 * in f64 on the JS side relative to that same origin (camera centre minus origin), and only that
 * small offset is passed to the shader as f32.
 */
import type { Batch } from "./batch";
import type { Camera } from "./camera";

export type Theme = "light" | "dark";

export type Renderer = {
  /** `null` clears the drawing. */
  setBatch(b: Batch | null): void;
  setHighlight(handles: Set<bigint>): void;
  draw(camera: Camera, theme: Theme): void;
  dispose(): void;
};

const rgb = (hex: number): [number, number, number] => [
  ((hex >> 16) & 255) / 255,
  ((hex >> 8) & 255) / 255,
  (hex & 255) / 255,
];
const INK = rgb(0x0e0f12);
const PAPER = rgb(0xf9f8f4);
const VIOLET = rgb(0x6e4a9e);
/** Highlight width in CSS pixels. */
const HIGHLIGHT_PX = 2;

const VERTEX = `#version 300 es
in vec2 a_pos;
in vec4 a_color;
uniform vec2 u_center;  // camera centre minus batch origin
uniform vec2 u_toClip;  // 2 / (canvas size in world units)
uniform vec2 u_offset;  // clip-space nudge, used to thicken the highlight
out vec4 v_color;
void main() {
  gl_Position = vec4((a_pos - u_center) * u_toClip + u_offset, 0.0, 1.0);
  v_color = a_color;
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in vec4 v_color;
uniform vec4 u_color;  // overrides the vertex colour when alpha > 0
uniform bool u_light;
uniform vec3 u_ink;
out vec4 outColor;
void main() {
  vec3 c = u_color.a > 0.0 ? u_color.rgb : v_color.rgb;
  // White lines (ACI 7 and friends) would vanish on Paper; draw them in Ink.
  if (u_light && all(greaterThan(c, vec3(0.999)))) c = u_ink;
  outColor = vec4(c, 1.0);
}`;

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(`Shader did not compile: ${gl.getShaderInfoLog(shader)}`);
  }
  return shader;
}

export function createRenderer(canvas: HTMLCanvasElement): Renderer {
  const context = canvas.getContext("webgl2", { antialias: true });
  if (!context) throw new Error("WebGL2 is not available");
  const gl = context;

  const program = gl.createProgram();
  const vs = compile(gl, gl.VERTEX_SHADER, VERTEX);
  const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(`Shader program did not link: ${gl.getProgramInfoLog(program)}`);
  }
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  const u = (name: string) => gl.getUniformLocation(program, name);
  const uCenter = u("u_center");
  const uToClip = u("u_toClip");
  const uOffset = u("u_offset");
  const uColor = u("u_color");
  const uLight = u("u_light");
  const uInk = u("u_ink");

  const vao = gl.createVertexArray();
  const positions = gl.createBuffer();
  const colors = gl.createBuffer();
  const highlightIndices = gl.createBuffer();
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, positions);
  const aPos = gl.getAttribLocation(program, "a_pos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
  gl.bindBuffer(gl.ARRAY_BUFFER, colors);
  const aColor = gl.getAttribLocation(program, "a_color");
  gl.enableVertexAttribArray(aColor);
  gl.vertexAttribPointer(aColor, 4, gl.UNSIGNED_BYTE, true, 0, 0);
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, highlightIndices);
  gl.bindVertexArray(null);

  let batch: Batch | null = null;
  let vertexCount = 0;
  let highlight = new Set<bigint>();
  let highlightCount = 0;

  function uploadHighlight() {
    const indices: number[] = [];
    for (const { handle, first, count } of batch?.ranges ?? []) {
      if (!highlight.has(handle)) continue;
      for (let v = 2 * first; v < 2 * (first + count); v++) indices.push(v);
    }
    highlightCount = indices.length;
    gl.bindVertexArray(vao);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint32Array(indices), gl.STATIC_DRAW);
    gl.bindVertexArray(null);
  }

  return {
    setBatch(b) {
      batch = b;
      const segmentColors = b?.colors ?? new Uint32Array();
      const n = segmentColors.length;
      vertexCount = 2 * n;
      // One RGBA byte quad per vertex; both ends of a segment share its colour.
      const rgba = new Uint8Array(8 * n);
      for (let i = 0; i < n; i++) {
        const c = segmentColors[i];
        for (const at of [8 * i, 8 * i + 4]) {
          rgba[at] = (c >> 16) & 255;
          rgba[at + 1] = (c >> 8) & 255;
          rgba[at + 2] = c & 255;
          rgba[at + 3] = 255;
        }
      }
      gl.bindBuffer(gl.ARRAY_BUFFER, positions);
      gl.bufferData(gl.ARRAY_BUFFER, b?.segments ?? new Float32Array(), gl.STATIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, colors);
      gl.bufferData(gl.ARRAY_BUFFER, rgba, gl.STATIC_DRAW);
      uploadHighlight();
    },

    setHighlight(handles) {
      highlight = handles;
      uploadHighlight();
    },

    draw(camera, theme) {
      const light = theme === "light";
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clearColor(...(light ? PAPER : INK), 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      if (!batch || vertexCount === 0) return;

      gl.useProgram(program);
      gl.bindVertexArray(vao);
      gl.uniform2f(uCenter, camera.cx - batch.origin[0], camera.cy - batch.origin[1]);
      gl.uniform2f(uToClip, 2 / (camera.width * camera.scale), 2 / (camera.height * camera.scale));
      gl.uniform1i(uLight, light ? 1 : 0);
      gl.uniform3f(uInk, ...INK);
      gl.uniform2f(uOffset, 0, 0);
      gl.uniform4f(uColor, 0, 0, 0, 0);
      gl.drawArrays(gl.LINES, 0, vertexCount);

      if (highlightCount > 0) {
        // Line width is 1 device pixel on most platforms, so thicken the highlight by redrawing
        // it nudged one device pixel at a time across and down.
        const dpr = canvas.width / camera.width;
        const steps = Math.max(2, Math.round(HIGHLIGHT_PX * dpr));
        const px = 2 / canvas.width;
        const py = 2 / canvas.height;
        gl.uniform4f(uColor, ...VIOLET, 1);
        for (let i = 0; i < steps; i++) {
          for (const [dx, dy] of i === 0 ? [[0, 0]] : [[i, 0], [0, -i]]) {
            gl.uniform2f(uOffset, dx * px, dy * py);
            gl.drawElements(gl.LINES, highlightCount, gl.UNSIGNED_INT, 0);
          }
        }
      }
      gl.bindVertexArray(null);
    },

    dispose() {
      // The context stays: React may mount the same canvas again (StrictMode), and a lost
      // context can't be recovered from the same element.
      gl.deleteBuffer(positions);
      gl.deleteBuffer(colors);
      gl.deleteBuffer(highlightIndices);
      gl.deleteVertexArray(vao);
      gl.deleteProgram(program);
    },
  };
}
