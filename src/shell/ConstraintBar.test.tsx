// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ConstraintInfo } from "../api";
import { LanguageProvider } from "../i18n";
import en from "../i18n/en.json";
import { byEntity, ConstraintBar, GLYPHS } from "./ConstraintBar";

afterEach(cleanup);

const c = (id: number, kind: ConstraintInfo["kind"], handles: number[], name: string | null = null): ConstraintInfo => ({
  id,
  kind,
  handles,
  at: [0, 0],
  name,
  expression: name ? "4" : null,
  value: name ? 4 : null,
  dimension: name ? 99 : null,
});

const LIST = [c(1, "horizontal", [10]), c(2, "coincident", [10, 11]), c(3, "linear", [11], "d1")];

test("glyphs are grouped by the entity each constraint holds first", () => {
  expect(byEntity(LIST).map(([h, l]) => [h, l.map((x) => x.id)])).toEqual([
    [10, [1, 2]],
    [11, [3]],
  ]);
});

test("a glyph highlights what it holds and deletes its constraint on click; nothing shows without any", () => {
  const onDelete = vi.fn();
  const onHover = vi.fn();
  const onHide = vi.fn();
  const { rerender } = render(
    <LanguageProvider>
      <ConstraintBar constraints={LIST} onDelete={onDelete} onHover={onHover} onHide={onHide} />
    </LanguageProvider>,
  );
  expect(screen.getAllByRole("group")).toHaveLength(2);
  const named = screen.getByRole("button", {
    name: en["param.glyphNamed"].replace("{kind}", en["param.kind.linear"]).replace("{name}", "d1"),
  });
  expect(named.textContent).toBe(`${GLYPHS.linear}d1`);
  const coincident = screen.getByRole("button", { name: en["param.glyph"].replace("{kind}", en["param.kind.coincident"]) });
  fireEvent.pointerEnter(coincident);
  expect(onHover).toHaveBeenLastCalledWith([10, 11]);
  fireEvent.pointerLeave(coincident);
  expect(onHover).toHaveBeenLastCalledWith(null);
  fireEvent.click(coincident);
  expect(onDelete).toHaveBeenCalledWith(2);
  fireEvent.click(screen.getByRole("button", { name: en["param.hideBar"] }));
  expect(onHide).toHaveBeenCalled();

  rerender(
    <LanguageProvider>
      <ConstraintBar constraints={[]} onDelete={onDelete} onHover={onHover} onHide={onHide} />
    </LanguageProvider>,
  );
  expect(screen.queryByRole("toolbar")).toBeNull();
});
