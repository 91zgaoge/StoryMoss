import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { Hero } from "../Hero";

vi.mock("framer-motion", async () => {
  const actual =
    await vi.importActual<typeof import("framer-motion")>("framer-motion");
  return {
    ...actual,
    useScroll: () => ({ scrollYProgress: { get: () => 0.5 } }),
    useTransform: (_v: unknown, _from: number[], to: number[]) => ({
      get: () => to[1],
    }),
  };
});

describe("Hero", () => {
  it("renders headline, version badge and CTAs", () => {
    render(<Hero />);
    expect(screen.getByText(/v0\.30\.0/)).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
      "让故事自己生长",
    );
    expect(
      screen.getByRole("link", {
        name: /免费下载|下载 macOS 版|下载 Windows 版|下载 Linux 版/i,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: /看看三个创作者/i }),
    ).toBeInTheDocument();
  });
});
