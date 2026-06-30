(function attachKeeprClipperExtractor(root) {
  const MAX_MARKDOWN_CHARS = 120 * 1024;
  const TEXT_NODE = 3;
  const ELEMENT_NODE = 1;
  const DOCUMENT_FRAGMENT_NODE = 11;
  const BLOCK_TAGS = new Set([
    "address",
    "article",
    "aside",
    "blockquote",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
  ]);
  const DROP_SELECTOR =
    "script,style,noscript,template,iframe,canvas,svg,nav,header,footer,aside,form,button,input,select,textarea,dialog,[hidden],[aria-hidden='true']";
  const NOISE_RE =
    /\b(ad|ads|advert|banner|breadcrumb|cookie|footer|header|menu|modal|nav|newsletter|promo|related|share|sidebar|social|sponsor|toolbar)\b/i;
  const ARTICLE_RE = /\b(article|body|content|entry|main|post|story)\b/i;

  function extractReadableClip(mode, fallback = {}) {
    const url = root.location?.href || fallback.url || "";
    const title =
      cleanText(meta("property", "og:title")) ||
      cleanText(meta("name", "twitter:title")) ||
      cleanText(root.document?.title) ||
      fallback.title ||
      url;
    const excerpt =
      cleanText(meta("name", "description")) ||
      cleanText(meta("property", "og:description")) ||
      cleanText(fallback.excerpt);

    if (mode === "selection") {
      const markdown = selectionMarkdown() || cleanText(fallback.selectionText);
      return {
        url,
        title,
        markdown: truncateMarkdown(markdown),
        excerpt,
      };
    }

    const candidate = pickArticleRoot();
    const markdown =
      (candidate ? markdownFromNode(cleanReadableClone(candidate), { baseUrl: url }) : "") ||
      textFallback(candidate || root.document?.body);

    return {
      url,
      title,
      markdown: truncateMarkdown(markdown),
      excerpt,
    };
  }

  function meta(attr, value) {
    return root.document
      ?.querySelector(`meta[${attr}="${value}"]`)
      ?.getAttribute("content");
  }

  function selectionMarkdown() {
    const selection = root.getSelection?.();
    if (!selection || selection.isCollapsed || selection.rangeCount === 0) return "";
    const holder = root.document.createElement("div");
    for (let i = 0; i < selection.rangeCount; i += 1) {
      holder.appendChild(selection.getRangeAt(i).cloneContents());
    }
    return markdownFromNode(holder, { baseUrl: root.location?.href || "" });
  }

  function pickArticleRoot() {
    const document = root.document;
    if (!document?.body) return null;
    const candidates = new Set([
      ...document.querySelectorAll(
        "article,main,[role='main'],[itemprop='articleBody'],.article,.article-body,.content,.entry-content,.post,.post-content,.story",
      ),
      document.body,
    ]);
    let best = document.body;
    let bestScore = scoreCandidate(best);
    for (const node of candidates) {
      const score = scoreCandidate(node);
      if (score > bestScore) {
        best = node;
        bestScore = score;
      }
    }
    return best;
  }

  function scoreCandidate(node) {
    const text = cleanText(node?.innerText || node?.textContent || "");
    if (!text) return 0;
    const name = `${node.tagName || ""} ${node.id || ""} ${node.className || ""}`;
    let score = Math.min(text.length / 80, 120);
    score += (node.querySelectorAll?.("p")?.length || 0) * 8;
    score += (node.querySelectorAll?.("h1,h2,h3")?.length || 0) * 3;
    if (/article/i.test(node.tagName || "")) score += 35;
    if (/main/i.test(node.tagName || "")) score += 20;
    if (ARTICLE_RE.test(name)) score += 25;
    if (NOISE_RE.test(name)) score -= 40;
    return score;
  }

  function cleanReadableClone(node) {
    const clone = node.cloneNode(true);
    clone.querySelectorAll?.(DROP_SELECTOR).forEach((el) => el.remove());
    clone.querySelectorAll?.("*").forEach((el) => {
      const label = `${el.id || ""} ${el.className || ""} ${el.getAttribute?.("role") || ""}`;
      const style = el.getAttribute?.("style") || "";
      if (
        NOISE_RE.test(label) ||
        /display\s*:\s*none/i.test(style) ||
        /visibility\s*:\s*hidden/i.test(style)
      ) {
        el.remove();
      }
    });
    return clone;
  }

  function markdownFromNode(node, context = {}) {
    if (!node) return "";
    if (node.nodeType === TEXT_NODE) {
      return String(node.nodeValue || "").replace(/\s+/g, " ");
    }
    if (node.nodeType === DOCUMENT_FRAGMENT_NODE) {
      return normalizeMarkdown(childrenMarkdown(node, context));
    }
    if (node.nodeType !== ELEMENT_NODE) {
      return "";
    }

    const tag = String(node.nodeName || "").toLowerCase();
    if (!tag || tag === "script" || tag === "style" || tag === "noscript") return "";

    if (/^h[1-6]$/.test(tag)) {
      const level = Number(tag.slice(1));
      return block(`${"#".repeat(level)} ${inlineMarkdown(node, context)}`);
    }
    if (tag === "p") return block(inlineMarkdown(node, context));
    if (tag === "br") return "  \n";
    if (tag === "hr") return "\n\n---\n\n";
    if (tag === "strong" || tag === "b") return wrapInline("**", inlineMarkdown(node, context));
    if (tag === "em" || tag === "i") return wrapInline("_", inlineMarkdown(node, context));
    if (tag === "code" && String(node.parentNode?.nodeName || "").toLowerCase() !== "pre") {
      const text = cleanText(node.textContent || "");
      return text ? "`" + text.replaceAll("`", "\\`") + "`" : "";
    }
    if (tag === "pre") {
      const text = (node.textContent || "").trim();
      return text ? `\n\n\`\`\`\n${text}\n\`\`\`\n\n` : "";
    }
    if (tag === "a") {
      const text = inlineMarkdown(node, context) || cleanText(node.textContent || "");
      const href = absoluteUrl(node.getAttribute?.("href") || node.href || "", context.baseUrl);
      if (!text) return href;
      if (!href || href.startsWith("javascript:")) return text;
      return `[${escapeLinkText(text)}](${href})`;
    }
    if (tag === "img") {
      const alt = cleanText(node.getAttribute?.("alt") || "");
      const src = absoluteUrl(node.getAttribute?.("src") || node.src || "", context.baseUrl);
      return alt && src ? `![${escapeLinkText(alt)}](${src})` : "";
    }
    if (tag === "blockquote") {
      const text = normalizeMarkdown(childrenMarkdown(node, context));
      return text ? block(text.split("\n").map((line) => `> ${line}`).join("\n")) : "";
    }
    if (tag === "ul" || tag === "ol") {
      return listMarkdown(node, context, tag === "ol");
    }
    if (tag === "table") {
      return tableMarkdown(node);
    }

    const text = childrenMarkdown(node, context);
    return BLOCK_TAGS.has(tag) ? block(text) : text;
  }

  function childrenMarkdown(node, context) {
    return Array.from(node.childNodes || [])
      .map((child) => markdownFromNode(child, context))
      .join("");
  }

  function inlineMarkdown(node, context) {
    return cleanInline(childrenMarkdown(node, context));
  }

  function listMarkdown(node, context, ordered) {
    const items = Array.from(node.childNodes || []).filter(
      (child) => child.nodeType === ELEMENT_NODE && String(child.nodeName).toLowerCase() === "li",
    );
    const markdown = items
      .map((item, index) => {
        const text = normalizeMarkdown(childrenMarkdown(item, context))
          .split("\n")
          .map((line, lineIndex) => (lineIndex === 0 ? line : `  ${line}`))
          .join("\n");
        if (!text) return "";
        return `${ordered ? `${index + 1}.` : "-"} ${text}`;
      })
      .filter(Boolean)
      .join("\n");
    return markdown ? `\n\n${markdown}\n\n` : "";
  }

  function tableMarkdown(node) {
    const rows = Array.from(node.querySelectorAll?.("tr") || [])
      .map((row) =>
        Array.from(row.querySelectorAll("th,td"))
          .map((cell) => cleanText(cell.textContent || "").replace(/\|/g, "\\|"))
          .filter(Boolean),
      )
      .filter((cells) => cells.length > 0);
    if (rows.length === 0) return "";
    const [head, ...body] = rows;
    const divider = head.map(() => "---");
    const allRows = [head, divider, ...body];
    return block(allRows.map((cells) => `| ${cells.join(" | ")} |`).join("\n"));
  }

  function textFallback(node) {
    const text = cleanText(node?.innerText || node?.textContent || "");
    if (!text) return "";
    return text
      .split(/\n{2,}/)
      .map((part) => part.trim())
      .filter(Boolean)
      .join("\n\n");
  }

  function block(text) {
    const cleaned = normalizeMarkdown(text);
    return cleaned ? `\n\n${cleaned}\n\n` : "";
  }

  function wrapInline(marker, text) {
    const cleaned = cleanInline(text);
    return cleaned ? `${marker}${cleaned}${marker}` : "";
  }

  function absoluteUrl(value, baseUrl) {
    const raw = cleanText(value);
    if (!raw) return "";
    try {
      return new URL(raw, baseUrl || root.location?.href || undefined).href;
    } catch (_e) {
      return raw;
    }
  }

  function truncateMarkdown(markdown) {
    const normalized = normalizeMarkdown(markdown);
    if (normalized.length <= MAX_MARKDOWN_CHARS) return normalized;
    return (
      normalized.slice(0, MAX_MARKDOWN_CHARS).trimEnd() +
      "\n\n[... truncated before sending to Keepr]"
    );
  }

  function normalizeMarkdown(value) {
    return String(value || "")
      .replace(/\r\n?/g, "\n")
      .replace(/\t/g, " ")
      .replace(/[ \t]+\n/g, "\n")
      .replace(/\n[ \t]+/g, "\n")
      .replace(/[ \t]{2,}/g, " ")
      .replace(/\n{3,}/g, "\n\n")
      .trim();
  }

  function cleanInline(value) {
    return cleanText(value).replace(/\s+/g, " ");
  }

  function cleanText(value) {
    return String(value || "")
      .replace(/\u00a0/g, " ")
      .replace(/[ \t]+/g, " ")
      .replace(/\n[ \t]+/g, "\n")
      .trim();
  }

  function escapeLinkText(value) {
    return cleanInline(value).replace(/[[\]]/g, "\\$&");
  }

  root.KeeprClipperExtractor = {
    absoluteUrl,
    extractReadableClip,
    markdownFromNode,
    normalizeMarkdown,
    truncateMarkdown,
  };
})(globalThis);
