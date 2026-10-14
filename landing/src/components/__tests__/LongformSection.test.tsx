import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { LongformSection } from "../LongformSection";

describe("LongformSection", () => {
  it("renders the four longform pillars", () => {
    render(<LongformSection />);
    expect(screen.getByText("写三百章，还认得前文")).toBeInTheDocument();
    expect(screen.getByText("写第 300 章，它去翻第 3 章")).toBeInTheDocument();
    expect(screen.getByText("卷与弧：长篇的中观路线")).toBeInTheDocument();
    expect(screen.getByText("改了旧章，下游不会悄悄过期")).toBeInTheDocument();
    expect(screen.getByText("死人不复活，秘密不提前说")).toBeInTheDocument();
  });
});
