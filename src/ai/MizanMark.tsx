// The Qiyas mark, alive: Mizan's "working" sign. The sphere's mesh is the same drawing as
// public/brand/qiyas-mark.svg; while Mizan works the sphere turns slowly and its face (the facet
// in the Qiyas colours) breathes. Pure CSS (theme.css, `.mizan-mark`); with reduced motion
// it stands still. Decorative: the text beside it says what is happening.
import { useId } from "react";

const SPHERE =
  "M18 109.32L31.22 171.34M18 109.32L31.63 138.83M18 109.32L41.86 84.2M18 109.32L43.74 51.39M31.22 171.34L31.63 138.83M31.22 171.34L63.25 184.55M31.22 171.34L78.35 213.75M31.63 138.83L41.86 84.2M31.63 138.83L63.25 184.55M31.63 138.83L69.83 130.69M41.86 84.2L43.74 51.39M41.86 84.2L69.83 130.69M41.86 84.2L75.43 40.26M41.86 84.2L95.57 72.76M43.74 51.39L75.43 40.26M43.74 51.39L98.61 19.66M63.25 184.55L69.83 130.69M63.25 184.55L78.35 213.75M63.25 184.55L111.82 210.98M63.25 184.55L116.96 173.1M69.83 130.69L95.57 72.76M69.83 130.69L116.96 173.1M69.83 130.69L123.01 119.36M75.43 40.26L95.57 72.76M75.43 40.26L98.61 19.66M75.43 40.26L130.64 32.86M78.35 213.75L111.82 210.98M78.35 213.75L141.39 220.34M95.57 72.76L123.01 119.36M95.57 72.76L130.64 32.86M95.57 72.76L158.61 79.35M98.61 19.66L130.64 32.86M98.61 19.66L161.65 26.25M111.82 210.98L116.96 173.1M111.82 210.98L141.39 220.34M111.82 210.98L165.25 195.22M116.96 173.1L123.01 119.36M116.96 173.1L165.25 195.22M116.96 173.1L171.83 141.37M123.01 119.36L158.61 79.35M123.01 119.36L171.83 141.37M130.64 32.86L158.61 79.35M130.64 32.86L161.65 26.25M130.64 32.86L182.68 51.49M141.39 220.34L165.25 195.22M141.39 220.34L196.26 188.61M158.61 79.35L171.83 141.37M158.61 79.35L182.68 51.49M158.61 79.35L206.9 101.48M161.65 26.25L182.68 51.49M161.65 26.25L208.78 68.66M165.25 195.22L171.83 141.37M165.25 195.22L196.26 188.61M165.25 195.22L205.17 157M171.83 141.37L205.17 157M171.83 141.37L206.9 101.48M182.68 51.49L206.9 101.48M182.68 51.49L208.78 68.66M196.26 188.61L205.17 157M196.26 188.61L222 130.68M205.17 157L206.9 101.48M205.17 157L222 130.68M206.9 101.48L208.78 68.66M206.9 101.48L222 130.68M208.78 68.66L222 130.68";
const FACE = "182.68,51.49 206.9,101.48 158.61,79.35";

export function MizanMark({ size = "sm", className }: { size?: "sm" | "md"; className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg
      className={["mizan-mark", `mizan-mark-${size}`, className].filter(Boolean).join(" ")}
      viewBox="0 0 240 240"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id={`mm-face-${id}`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#C9A6E0" />
          <stop offset="1" stopColor="#6E4A9E" />
        </linearGradient>
      </defs>
      <g className="mm-turn">
        <path className="mm-sphere" d={SPHERE} />
        <polygon className="mm-face" points={FACE} fill={`url(#mm-face-${id})`} />
      </g>
    </svg>
  );
}
