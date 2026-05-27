import { describe, expect, it } from "vitest";
import { findPartialHashtag } from "../components/NoteEditor";

describe("findPartialHashtag", () => {
  it("returns null when caret is not after a hashtag", () => {
    expect(findPartialHashtag("hello world", 5)).toBeNull();
    expect(findPartialHashtag("", 0)).toBeNull();
  });

  it("detects a bare # at the caret as an empty-prefix completion", () => {
    expect(findPartialHashtag("hello #", 7)).toEqual({
      prefix: "",
      start: 6,
      end: 7,
    });
  });

  it("detects #wo with the caret at the end", () => {
    expect(findPartialHashtag("hello #wo", 9)).toEqual({
      prefix: "wo",
      start: 6,
      end: 9,
    });
  });

  it("does NOT trigger inside a mid-word #", () => {
    // foo#bar should NOT be a hashtag — # must be preceded by non-word.
    expect(findPartialHashtag("foo#bar", 7)).toBeNull();
  });

  it("triggers at start-of-string", () => {
    expect(findPartialHashtag("#work", 5)).toEqual({
      prefix: "work",
      start: 0,
      end: 5,
    });
  });

  it("triggers on a new line", () => {
    const text = "hello\n#sec";
    expect(findPartialHashtag(text, text.length)).toEqual({
      prefix: "sec",
      start: 6,
      end: 10,
    });
  });

  it("returns null when caret is past a space after the hashtag", () => {
    // After typing space, the hashtag is complete and the caret is
    // outside the token. No completion shown.
    expect(findPartialHashtag("#work ", 6)).toBeNull();
  });
});
