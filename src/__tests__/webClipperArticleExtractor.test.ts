import { beforeAll, describe, expect, it } from "vitest";

type Extractor = {
  absoluteUrl(value: string, baseUrl?: string): string;
  markdownFromNode(node: unknown, context?: { baseUrl?: string }): string;
  normalizeMarkdown(value: string): string;
  truncateMarkdown(value: string): string;
};

declare global {
  // The extension script attaches its API to globalThis for content-script use.
  // Tests import that same script and read the same global.
  var KeeprClipperExtractor: Extractor;
}

beforeAll(async () => {
  // @ts-expect-error - this is a browser-extension script outside the TS project include.
  await import("../../web-clipper/article-extractor.js");
});

const TEXT_NODE = 3;
const ELEMENT_NODE = 1;

type FakeNode = {
  nodeType: number;
  nodeName?: string;
  nodeValue?: string;
  textContent?: string;
  childNodes?: FakeNode[];
  parentNode?: FakeNode;
  href?: string;
  src?: string;
  getAttribute?(key: string): string | null;
  querySelectorAll?(): FakeNode[];
};

function text(value: string): FakeNode {
  return {
    nodeType: TEXT_NODE,
    nodeValue: value,
    textContent: value,
  };
}

function el(name: string, attrs: Record<string, string> = {}, children: FakeNode[] = []): FakeNode {
  const node: FakeNode = {
    nodeType: ELEMENT_NODE,
    nodeName: name.toUpperCase(),
    childNodes: children,
    href: attrs.href,
    src: attrs.src,
    getAttribute(key: string) {
      return attrs[key] ?? null;
    },
    querySelectorAll() {
      return [];
    },
  };
  for (const child of children) {
    child.parentNode = node;
  }
  Object.defineProperty(node, "textContent", {
    get() {
      return children.map((child) => child.textContent || child.nodeValue || "").join("");
    },
  });
  return node;
}

function extractor() {
  return globalThis.KeeprClipperExtractor;
}

describe("Keepr Web Clipper article extractor", () => {
  it("turns common article nodes into readable Markdown", () => {
    const tree = el("article", {}, [
      el("h1", {}, [text("Launch Notes")]),
      el("p", {}, [
        text("Read the "),
        el("a", { href: "/guide" }, [text("guide")]),
        text(" first."),
      ]),
      el("ul", {}, [
        el("li", {}, [el("strong", {}, [text("Fast")]), text(" local saves")]),
        el("li", {}, [text("No cloud account")]),
      ]),
    ]);

    const markdown = extractor().markdownFromNode(tree, {
      baseUrl: "https://example.com/articles/launch",
    });

    expect(markdown).toContain("# Launch Notes");
    expect(markdown).toContain("[guide](https://example.com/guide)");
    expect(markdown).toContain("- **Fast** local saves");
    expect(markdown).toContain("- No cloud account");
  });

  it("normalizes whitespace and caps oversized Markdown before POST", () => {
    expect(extractor().normalizeMarkdown(" One  \n\n\n  Two\tTwo ")).toBe("One\n\nTwo Two");

    const long = "x".repeat(130 * 1024);
    const truncated = extractor().truncateMarkdown(long);
    expect(truncated.length).toBeLessThan(long.length);
    expect(truncated).toContain("[... truncated before sending to Keepr]");
  });

  it("resolves relative URLs against the page source", () => {
    expect(extractor().absoluteUrl("../next", "https://example.com/a/b/c")).toBe(
      "https://example.com/a/next",
    );
  });
});
