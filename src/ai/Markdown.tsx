import type { ReactNode } from "react";

/** `**bold**` and `code` inside a line, as text nodes only. */
function inline(text: string): ReactNode[] {
  return text.split(/(\*\*[^*\n]+\*\*|`[^`\n]+`)/g).map((part, i) => {
    if (part.length > 4 && part.startsWith("**") && part.endsWith("**")) {
      return <strong key={i}>{part.slice(2, -2)}</strong>;
    }
    if (part.length > 2 && part.startsWith("`") && part.endsWith("`")) {
      return <code key={i}>{part.slice(1, -1)}</code>;
    }
    return part;
  });
}

/** Markdown-lite: paragraphs (blank lines between), `-` bullets, bold and inline code. Never HTML. */
export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let para: string[] = [];
  let bullets: string[] = [];
  const flush = () => {
    if (para.length > 0) {
      const lines = para;
      blocks.push(
        <p key={blocks.length} dir="auto">
          {lines.map((l, i) => (
            <span key={i}>
              {i > 0 && <br />}
              {inline(l)}
            </span>
          ))}
        </p>,
      );
    }
    if (bullets.length > 0) {
      blocks.push(
        <ul key={blocks.length} dir="auto">
          {bullets.map((b, i) => (
            <li key={i}>{inline(b)}</li>
          ))}
        </ul>,
      );
    }
    para = [];
    bullets = [];
  };
  for (const line of text.split("\n")) {
    const bullet = /^\s*[-*]\s+(.*)$/.exec(line);
    if (bullet) {
      if (para.length > 0) flush();
      bullets.push(bullet[1]);
    } else if (!line.trim()) {
      flush();
    } else {
      if (bullets.length > 0) flush();
      para.push(line);
    }
  }
  flush();
  return <>{blocks}</>;
}
