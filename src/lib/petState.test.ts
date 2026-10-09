import { describe, expect, it } from "vitest";

import { derivePetState, type PetStateSignals } from "./petState";

function signals(overrides: Partial<PetStateSignals> = {}): PetStateSignals {
  return {
    chatOpen: true,
    needsSettings: false,
    error: "",
    connection: "connected",
    loadingHistory: false,
    queueLength: 0,
    streamPhase: "idle",
    hasAssistantText: false,
    ...overrides,
  };
}

describe("derivePetState", () => {
  it("listens while the chat is open and waiting for input", () => {
    expect(derivePetState(signals())).toBe("listening");
  });

  it("rests when the chat window is closed", () => {
    expect(derivePetState(signals({ chatOpen: false }))).toBe("idle");
  });

  it("reports setup and connection failures as errors", () => {
    expect(derivePetState(signals({ needsSettings: true }))).toBe("error");
    expect(derivePetState(signals({ error: "连接失败" }))).toBe("error");
    expect(derivePetState(signals({ connection: "disconnected" }))).toBe(
      "error",
    );
  });

  it("waits while messages are queued", () => {
    expect(
      derivePetState(signals({ queueLength: 2, connection: "streaming" })),
    ).toBe("waiting");
  });

  it("thinks until the first token and works once text arrives", () => {
    expect(
      derivePetState(
        signals({ connection: "streaming", streamPhase: "generating" }),
      ),
    ).toBe("thinking");
    expect(
      derivePetState(
        signals({
          connection: "streaming",
          streamPhase: "generating",
          hasAssistantText: true,
        }),
      ),
    ).toBe("working");
  });

  it("works while a tool runs even before any text", () => {
    expect(
      derivePetState(signals({ connection: "streaming", streamPhase: "tool" })),
    ).toBe("working");
  });

  it("thinks while connecting or loading history", () => {
    expect(derivePetState(signals({ connection: "loading" }))).toBe("thinking");
    expect(derivePetState(signals({ loadingHistory: true }))).toBe("thinking");
  });

  it("ranks failures above queued and running work", () => {
    expect(
      derivePetState(
        signals({ error: "boom", queueLength: 1, connection: "streaming" }),
      ),
    ).toBe("error");
  });
});
