import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { QuickStart } from "../QuickStart";

describe("QuickStart", () => {
  it("renders three steps", () => {
    render(<QuickStart />);
    expect(screen.getByText("装上你的模型")).toBeInTheDocument();
    expect(screen.getByText("一句话创世")).toBeInTheDocument();
    expect(screen.getByText("看着它长")).toBeInTheDocument();
  });
});
