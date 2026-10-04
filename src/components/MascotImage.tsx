import {
  mascotAnimationSource,
  type TaijiAnimationState,
} from "../lib/mascotAnimation";

interface MascotImageProps {
  src: string;
  animationState?: TaijiAnimationState;
}

export default function MascotImage({
  src,
  animationState = "idle",
}: MascotImageProps) {
  return (
    <img
      className="mascot-image"
      src={mascotAnimationSource(src, animationState)}
      alt={src.includes("/taiji-bot/") ? "太极机器人" : "Octop 宠物"}
      draggable={false}
    />
  );
}
