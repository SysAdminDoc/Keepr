import { describe, expect, it } from "vitest";
import { useStore } from "../store";

describe("Zustand store module", () => {
  it("keeps one shared store across static and dynamic ESM imports", async () => {
    const originalSearch = useStore.getState().search;

    try {
      useStore.setState({ search: "zustand-esm-guard" });

      const imported = await import("../store");

      expect(imported.useStore).toBe(useStore);
      expect(imported.useStore.getState().search).toBe("zustand-esm-guard");

      imported.useStore.getState().setSearch("zustand-action-guard");
      expect(useStore.getState().search).toBe("zustand-action-guard");
    } finally {
      useStore.setState({ search: originalSearch });
    }
  });
});
