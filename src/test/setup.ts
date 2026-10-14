/** Test setup for every file: browser APIs jsdom does not have and the libraries behind the
 *  window's lists and tabs call (cmdk and TanStack Virtual watch sizes; Base UI measures). A test
 *  that needs real sizes mocks them itself. */
if (typeof window !== "undefined") {
  if (!("ResizeObserver" in globalThis)) {
    class ResizeObserverStub {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
    (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
  }
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => {};
  }
}
