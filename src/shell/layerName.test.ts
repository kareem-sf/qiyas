import { expect, test } from "vitest";
import { shortLayerName } from "./layerName";

test("a bound xref layer is shown by its own name, as the core does", () => {
  expect(shortLayerName("A-WALL")).toBe("A-WALL");
  expect(shortLayerName("BASE-B$0$A-WALL")).toBe("A-WALL");
  expect(shortLayerName("02-First floor (arch&furn)$0$02-FIRST FLOOR (ARCH&FURN)$0$A-WALL")).toBe(
    "A-WALL",
  );
  expect(shortLayerName("X$12$Y")).toBe("Y");
  // `$` without the `$N$` pattern stays.
  expect(shortLayerName("COST$A-WALL")).toBe("COST$A-WALL");
  expect(shortLayerName("A$$B$x$C")).toBe("A$$B$x$C");
  expect(shortLayerName("A$0$")).toBe("A$0$");
});
