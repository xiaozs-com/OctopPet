import type { TaijiAnimationState } from "./mascotAnimation";
import type { StreamStatusPhase } from "./streamStatus";

/** Chat signals that drive the pet's animation. */
export interface PetStateSignals {
  /** Whether the chat window is currently on screen. */
  chatOpen: boolean;
  needsSettings: boolean;
  error: string;
  connection: "loading" | "connected" | "disconnected" | "streaming";
  loadingHistory: boolean;
  queueLength: number;
  streamPhase: StreamStatusPhase;
  hasAssistantText: boolean;
}

/**
 * Map chat activity onto the pet's animation state.
 *
 * Order matters: a failure outranks activity, queued work outranks a live
 * stream, and a streaming reply is "thinking" until its first token lands.
 * Only the resting case depends on whether the chat window is on screen, so a
 * hidden chat leaves the pet idle rather than apparently listening.
 */
export function derivePetState(signals: PetStateSignals): TaijiAnimationState {
  const {
    chatOpen,
    needsSettings,
    error,
    connection,
    loadingHistory,
    queueLength,
    streamPhase,
    hasAssistantText,
  } = signals;

  if (needsSettings || error !== "" || connection === "disconnected") {
    return "error";
  }
  if (queueLength > 0) return "waiting";
  if (connection === "streaming") {
    return streamPhase === "tool" || hasAssistantText ? "working" : "thinking";
  }
  if (connection === "loading" || loadingHistory) return "thinking";
  return chatOpen ? "listening" : "idle";
}
