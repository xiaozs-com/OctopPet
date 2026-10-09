// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import MascotImage from "./MascotImage";

afterEach(cleanup);
describe("Taiji default idle animation", () => {
  it("uses a static body with a separate eye overlay", () => {
    const { container } = render(
      <MascotImage src="/mascots/taiji-bot/idle.webp" />,
    );
    expect(screen.getByRole("img", { name: "太极机器人" })).toHaveAttribute(
      "src",
      "/mascots/taiji-bot/idle.png",
    );
    expect(container.querySelector(".taiji-idle-eyes")).toBeInTheDocument();
    expect(container.querySelectorAll("img")).toHaveLength(1);
  });
  it("preserves other task animations", () => {
    const { container } = render(
      <MascotImage
        src="/mascots/taiji-bot/idle.webp"
        animationState="working"
      />,
    );
    expect(screen.getByRole("img")).toHaveAttribute(
      "src",
      "/mascots/taiji-bot/working.webp",
    );
    expect(container.querySelector(".taiji-idle-eyes")).toBeNull();
  });
  it("preserves the other mascot", () => {
    const { container } = render(<MascotImage src="/mascots/peek.webp" />);
    expect(screen.getByRole("img")).toHaveAttribute(
      "src",
      "/mascots/peek.webp",
    );
    expect(container.querySelector(".taiji-idle-eyes")).toBeNull();
  });
});
