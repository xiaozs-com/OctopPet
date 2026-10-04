export const TAIJI_ANIMATION_STATES = [
  "idle",
  "waving",
  "listening",
  "thinking",
  "working",
  "waiting",
  "success",
  "error",
] as const;

export type TaijiAnimationState = (typeof TAIJI_ANIMATION_STATES)[number];

export function mascotAnimationSource(
  src: string,
  state: TaijiAnimationState = "idle",
): string {
  return src === "/mascots/taiji-bot/idle.webp"
    ? `/mascots/taiji-bot/${state}.webp`
    : src;
}
