import "../styles/mascot.css";
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
  if (
    mascotAnimationSource(src, animationState) ===
    "/mascots/taiji-bot/idle.webp"
  ) {
    return (
      <span className="mascot-image taiji-idle">
        <img
          src="/mascots/taiji-bot/idle.png"
          alt="太极机器人"
          draggable={false}
        />
        <svg viewBox="0 0 222 222" aria-hidden="true" focusable="false">
          <g className="taiji-idle-eyes">
            <ellipse cx="88" cy="103" rx="18" ry="19" fill="#fff" />
            <ellipse cx="154" cy="103" rx="18" ry="19" fill="#fff" />
            <path
              d="M 77 104 Q 88 94 99 104 M 143 104 Q 154 94 165 104"
              fill="none"
              stroke="#171717"
              strokeWidth="3.5"
              strokeLinecap="round"
            />
          </g>
        </svg>
      </span>
    );
  }
  return (
    <img
      className="mascot-image"
      src={mascotAnimationSource(src, animationState)}
      alt={src.includes("/taiji-bot/") ? "太极机器人" : "Octop 宠物"}
      draggable={false}
    />
  );
}
