import { describe, expect, it } from "vitest";

import {
  choosePasteRoute,
  hasMultipleParagraphs,
  looksLikeOutline,
  utf16OffsetToCharOffset,
} from "./index";

describe("hasMultipleParagraphs", () => {
  it("is false for a single line", () => {
    expect(hasMultipleParagraphs("just one line")).toBe(false);
    expect(hasMultipleParagraphs("https://example.com/x")).toBe(false);
  });

  it("is true for multiple non-blank lines (single or blank separators)", () => {
    // A chat reply arrives one line per paragraph, `\n`-separated.
    expect(hasMultipleParagraphs("line a\nline b\nline c")).toBe(true);
    expect(hasMultipleParagraphs("para one\n\npara two")).toBe(true);
  });

  it("ignores leading / trailing blank lines", () => {
    expect(hasMultipleParagraphs("\n\nsolo\n\n")).toBe(false);
  });

  it("treats whitespace-only lines as blank (not a second paragraph)", () => {
    expect(hasMultipleParagraphs("solo\n   \n\t")).toBe(false);
    expect(hasMultipleParagraphs("a\n   \nb")).toBe(true);
  });

  it("counts CRLF-separated lines (Windows clipboards)", () => {
    expect(hasMultipleParagraphs("line a\r\nline b")).toBe(true);
    expect(hasMultipleParagraphs("solo\r\n")).toBe(false);
  });

  it("is true at exactly two non-blank lines (the boundary)", () => {
    expect(hasMultipleParagraphs("one\ntwo")).toBe(true);
  });
});

describe("looksLikeOutline", () => {
  it("returns false for empty input", () => {
    expect(looksLikeOutline("")).toBe(false);
  });

  it("returns false for plain text", () => {
    expect(looksLikeOutline("just one line of text")).toBe(false);
    expect(looksLikeOutline("multi\nline\nbut no bullets")).toBe(false);
  });

  it("returns true on a single bullet line", () => {
    expect(looksLikeOutline("- one bullet")).toBe(true);
  });

  it("returns true when the bullet is indented", () => {
    expect(looksLikeOutline("    - nested")).toBe(true);
    expect(looksLikeOutline("\t- tab-indented")).toBe(true);
  });

  it("returns true when bullets appear after non-bullet preface", () => {
    expect(looksLikeOutline("intro paragraph\n- bullet")).toBe(true);
  });

  it("ignores leading whitespace lines", () => {
    expect(looksLikeOutline("\n\n  \n- after blanks")).toBe(true);
  });

  it("treats an empty bullet marker as outline", () => {
    expect(looksLikeOutline("-")).toBe(true);
    expect(looksLikeOutline("  -")).toBe(true);
  });

  it("returns false for dash followed by non-space", () => {
    expect(looksLikeOutline("-foo")).toBe(false);
    expect(looksLikeOutline("hyphen-word")).toBe(false);
  });
});

describe("utf16OffsetToCharOffset", () => {
  it("returns 0 for offset 0", () => {
    expect(utf16OffsetToCharOffset("anything", 0)).toBe(0);
    expect(utf16OffsetToCharOffset("", 0)).toBe(0);
  });

  it("matches the UTF-16 offset for pure ASCII", () => {
    const s = "hello world";
    expect(utf16OffsetToCharOffset(s, 5)).toBe(5);
    expect(utf16OffsetToCharOffset(s, s.length)).toBe(s.length);
  });

  it("matches the UTF-16 offset for BMP text", () => {
    // pt-BR with accents — `á` is U+00E1, still BMP, one code unit.
    const s = "olá mundo";
    expect(utf16OffsetToCharOffset(s, 4)).toBe(4); // after "olá "
    expect(utf16OffsetToCharOffset(s, s.length)).toBe(s.length);
  });

  it("collapses surrogate pairs to a single char", () => {
    // 😀 = U+1F600 — supplementary plane, takes 2 UTF-16 code units.
    const s = "hi 😀 you";
    expect(utf16OffsetToCharOffset(s, 5)).toBe(4);
    expect(s.length).toBe(9);
    expect(utf16OffsetToCharOffset(s, s.length)).toBe(8);
  });

  it("clamps when the offset overshoots", () => {
    const s = "abc";
    expect(utf16OffsetToCharOffset(s, 999)).toBe(3);
  });
});

describe("choosePasteRoute", () => {
  it("routes rich when HTML adds formatting the plain text lacks", () => {
    const d = choosePasteRoute("<b>bold</b> word", "bold word");
    expect(d).toEqual({ route: "rich", text: "**bold** word" });
  });

  it("does NOT round-trip when HTML is just a styled wrapper (md === plain)", () => {
    // A <span> with no markdown-visible formatting converts to the same
    // text as the plain flavour → not rich; a single line → native.
    const d = choosePasteRoute("<span>hello world</span>", "hello world");
    expect(d).toEqual({ route: "native" });
  });

  it("treats an alt-less image (md empty) as non-rich, falls to plain", () => {
    // md === "" → not rich; single-line plain → native.
    expect(choosePasteRoute('<img src="x.png">', "a url")).toEqual({
      route: "native",
    });
  });

  it("routes structured for a plain outline with no richer HTML", () => {
    expect(choosePasteRoute("", "- one\n- two")).toEqual({
      route: "structured",
      text: "- one\n- two",
    });
  });

  it("routes structured for multi-paragraph plain text", () => {
    const plain = "First line.\nSecond line.";
    expect(choosePasteRoute("", plain)).toEqual({
      route: "structured",
      text: plain,
    });
  });

  it("routes native for trivial single-line plain text", () => {
    expect(choosePasteRoute("", "https://example.com")).toEqual({
      route: "native",
    });
    expect(choosePasteRoute("", "")).toEqual({ route: "native" });
  });

  it("ignores trailing whitespace when comparing HTML vs plain", () => {
    // Plain has a trailing newline the HTML doesn't; the trimmed compare
    // must not flag it as rich when the content is identical.
    expect(choosePasteRoute("<span>hi</span>", "hi\n")).toEqual({
      route: "native",
    });
  });

  it("prefers rich over structured when both HTML and multi-paragraph plain exist", () => {
    const d = choosePasteRoute(
      "<p><b>H</b></p><p>body</p>",
      "H\nbody",
    );
    expect(d.route).toBe("rich");
    if (d.route === "rich") expect(d.text).toContain("**H**");
  });
});
