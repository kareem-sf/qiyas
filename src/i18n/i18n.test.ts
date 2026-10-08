// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import ar from "./ar.json";
import en from "./en.json";
import { LanguageProvider, useLanguage, useT } from "./index";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

test("en and ar have the same keys", () => {
  expect(Object.keys(ar).sort()).toEqual(Object.keys(en).sort());
});

test("arabic sets rtl", () => {
  const { result } = renderHook(() => useLanguage(), { wrapper: LanguageProvider });
  expect(document.documentElement.dir).toBe("ltr");
  act(() => result.current.setLang("ar"));
  expect(document.documentElement.dir).toBe("rtl");
  expect(document.documentElement.lang).toBe("ar");
});

test("t interpolates and falls back to the key", () => {
  const { result } = renderHook(() => useT(), { wrapper: LanguageProvider });
  expect(result.current("recovered", { n: 3 })).toBe("Recovered with 3 problems");
  expect(result.current("no.such.key")).toBe("no.such.key");
});
