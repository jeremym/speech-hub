import { describe, expect, it } from "vitest";
import { eventToShortcut, formatShortcut } from "./shortcut";

const ev = (code: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {}) =>
  ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

describe("eventToShortcut", () => {
  it("builds Alt+Escape", () => expect(eventToShortcut(ev("Escape", { altKey: true }))).toBe("Alt+Escape"));
  it("orders modifiers", () => expect(eventToShortcut(ev("KeyS", { shiftKey: true, altKey: true, metaKey: true }))).toBe("Alt+Shift+Super+S"));
  it("ignores modifier-only presses", () => expect(eventToShortcut(ev("ShiftLeft", { shiftKey: true }))).toBeNull());
  it("refuses bare letters", () => expect(eventToShortcut(ev("KeyA"))).toBeNull());
  it("allows bare function keys", () => expect(eventToShortcut(ev("F5"))).toBe("F5"));
});

describe("formatShortcut", () => {
  it("renders mac symbols", () => expect(formatShortcut("Alt+Shift+Escape")).toBe("⌥⇧⎋"));
  it("normalizes aliases", () => expect(formatShortcut("Cmd+Option+K")).toBe("⌥⌘K"));
});
