import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";
import { Navbar } from "../Navbar";

describe("Navbar", () => {
  it("renders logo and brand", () => {
    render(<Navbar />);
    expect(screen.getByAltText("StoryMoss 草苔")).toBeInTheDocument();
    expect(screen.getByText("草苔")).toBeInTheDocument();
    expect(screen.getByText("StoryMoss")).toBeInTheDocument();
  });

  it("renders links on desktop", () => {
    render(<Navbar />);
    expect(screen.getByRole("link", { name: /协作/i })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /免费下载/i })).toBeInTheDocument();
  });

  it("toggles mobile menu", async () => {
    render(<Navbar />);
    const toggle = screen.getByLabelText(/打开菜单/i);
    await userEvent.click(toggle);
    expect(screen.getByLabelText(/关闭菜单/i)).toBeInTheDocument();
  });
});
