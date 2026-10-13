// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CommentInfo } from "../api";
import { CommentsPanel } from "./CommentsPanel";

const comment: CommentInfo = {
  id: "C2A",
  handle: 42,
  label: 43,
  text: "Check the door width",
  sheet: "*Paper_Space",
  bounds: [
    [0, 0],
    [4, 3],
  ],
};
const sheets = [
  { name: "Model", block: "*Model_Space", is_model: true },
  { name: "Layout1", block: "*Paper_Space", is_model: false },
];

afterEach(cleanup);

describe("CommentsPanel", () => {
  it("lists each comment with its sheet and shows it on a click", () => {
    const onShow = vi.fn();
    render(
      <CommentsPanel comments={[comment]} sheets={sheets} onShow={onShow} onRefresh={() => {}} />
    );
    expect(screen.getByText("Layout1")).toBeTruthy();
    fireEvent.click(screen.getByText(/Check the door width/));
    expect(onShow).toHaveBeenCalledWith(comment);
  });

  it("says how to draw one when there are none", () => {
    render(
      <CommentsPanel comments={[]} sheets={sheets} onShow={() => {}} />
    );
    expect(screen.getByText(/No comments yet/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Refresh" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
