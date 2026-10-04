import { describe, expect, it } from "vitest";
import manifest from "../../public/mascots/taiji-bot/manifest.json";
import {
  mascotAnimationSource,
  TAIJI_ANIMATION_STATES,
} from "./mascotAnimation";
import { MASCOT_SRC } from "./configLogic";

describe("Taiji mascot assets", () => {
  it("matches the animation manifest for every selectable state", () => {
    expect(Object.keys(manifest.states)).toEqual([...TAIJI_ANIMATION_STATES]);
    for (const state of TAIJI_ANIMATION_STATES) {
      expect(mascotAnimationSource(MASCOT_SRC["taiji-bot"], state)).toBe(
        `/mascots/taiji-bot/${manifest.states[state].animation}`,
      );
    }
  });

  it("keeps existing mascot paths when a task state is supplied", () => {
    expect(mascotAnimationSource(MASCOT_SRC.peek, "working")).toBe(
      MASCOT_SRC.peek,
    );
    expect(mascotAnimationSource(MASCOT_SRC.type, "error")).toBe(
      MASCOT_SRC.type,
    );
  });
});
