import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import App from "./App";

describe("SyncHalo shell", () => {
  beforeEach(() => {
    cleanup();
    window.localStorage.clear();
  });

  it("renders the two-column shell and navigates between the three pages", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "粘贴板历史" })).toBeInTheDocument();
    expect(screen.getByText("3 台在线")).toBeInTheDocument();
    expect(screen.getByText("1 台离线")).toBeInTheDocument();
    const clipboardHeader = document.querySelector<HTMLElement>(".clipboard-page .page-header");
    expect(clipboardHeader).not.toBeNull();
    expect(
      within(clipboardHeader!).queryByRole("button", { name: /暂停同步|恢复同步/ }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));
    expect(screen.getByRole("heading", { name: "同步文件" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /设置/ }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();
    expect(screen.getByText("我的设备")).toBeInTheDocument();
    expect(screen.getByText("同步与历史")).toBeInTheDocument();
    expect(screen.queryByText("粘贴板与历史")).not.toBeInTheDocument();
    expect(screen.queryByText("自动同步粘贴板")).not.toBeInTheDocument();
    expect(screen.queryByText("平台能力")).not.toBeInTheDocument();
    expect(screen.queryByText(/Protocol v1/i)).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "删除同步" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "收藏同步" })).not.toBeChecked();
    fireEvent.click(screen.getByRole("switch", { name: "删除同步" }));
    fireEvent.click(screen.getByRole("switch", { name: "收藏同步" }));
    await waitFor(() => {
      expect(screen.getByRole("switch", { name: "删除同步" })).toBeChecked();
      expect(screen.getByRole("switch", { name: "收藏同步" })).toBeChecked();
    });
    for (const removedDescription of [
      "用一次性短码把同一局域网内的设备加入你的同步空间。",
      "只有这里列出的可信设备可以接收内容。",
      "收到的文件会在完整校验后才以最终名称出现。",
      "历史正文使用本机安全存储中的密钥加密。",
      "控制应用在系统中的常驻方式。",
      "设备名称会显示给同一同步空间中的其他设备。",
    ]) {
      expect(screen.queryByText(removedDescription)).not.toBeInTheDocument();
    }
    expect(screen.queryByText("传输完成与错误通知")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("加入另一台设备")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^加入$/ }));
    const joinDialog = screen.getByRole("dialog", { name: "加入另一台设备" });
    expect(joinDialog).toHaveClass("dialog-backdrop");
    const joinInput = screen.getByLabelText("输入一次性同步码");
    fireEvent.change(joinInput, { target: { value: "482913" } });
    fireEvent.click(screen.getByRole("button", { name: "加入设备" }));
    await waitFor(() => expect(joinDialog).not.toBeInTheDocument());
  });

  it("suppresses the browser context menu", () => {
    const { unmount } = render(<App />);
    const contextMenuEvent = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
    });

    document.body.dispatchEvent(contextMenuEvent);
    expect(contextMenuEvent.defaultPrevented).toBe(true);

    unmount();
    const eventAfterUnmount = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
    });
    document.body.dispatchEvent(eventAfterUnmount);
    expect(eventAfterUnmount.defaultPrevented).toBe(false);
  });

  it("paginates 100 items per page and toggles the favorites filter", async () => {
    const { container } = render(<App />);
    expect(await screen.findByText("第 1 / 3 页 · 共 205 条 · 每页 100 条")).toBeInTheDocument();

    const scroller = container.querySelector<HTMLElement>(".clipboard-page .page-scroll");
    expect(scroller).not.toBeNull();
    scroller!.scrollTop = 640;
    fireEvent.click(screen.getByRole("button", { name: "下一页" }));
    expect(scroller!.scrollTop).toBe(0);
    expect(await screen.findByText("第 2 / 3 页 · 共 205 条 · 每页 100 条")).toBeInTheDocument();
    expect(await screen.findByText("历史记录 #101 · SyncHalo 分页数据")).toBeInTheDocument();

    scroller!.scrollTop = 480;
    fireEvent.click(
      within(screen.getByRole("navigation", { name: "粘贴板历史分页" })).getByRole("button", {
        name: "1",
      }),
    );
    expect(scroller!.scrollTop).toBe(0);
    expect(await screen.findByText("第 1 / 3 页 · 共 205 条 · 每页 100 条")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "只看收藏" }));
    expect(await screen.findByText("共 5 条")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "显示全部历史" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "显示全部历史" }));
    expect(await screen.findByText("第 1 / 3 页 · 共 205 条 · 每页 100 条")).toBeInTheDocument();
  });

  it("does not copy when the history content itself is clicked", async () => {
    render(<App />);
    const content = await screen.findByText("cargo test --workspace");
    fireEvent.click(content);
    expect(screen.queryByText("已复制")).not.toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "复制这条历史" })[0]);
    expect(await screen.findByText("已复制")).toBeInTheDocument();
  });

  it("shows the full clipboard content in a dialog", async () => {
    render(<App />);
    const content = await screen.findByText(/会议结论：MVP 首发覆盖/);
    const row = content.closest("article");
    expect(row).not.toBeNull();

    fireEvent.click(within(row!).getByRole("button", { name: "查看完整内容" }));
    const dialog = screen.getByRole("dialog", { name: "完整内容" });
    expect(within(dialog).getByLabelText("完整剪贴板内容")).toHaveValue(
      "会议结论：MVP 首发覆盖 macOS 和 Ubuntu ARM64。\n文件流不经过 WebView，历史正文在本地加密。",
    );
  });

  it("shows a filled star before the source name only for favorited history", async () => {
    render(<App />);
    const regularRow = (await screen.findByText("cargo test --workspace")).closest("article");
    const favoriteRow = screen.getByText("https://github.com/tauri-apps/tauri").closest("article");

    expect(regularRow).not.toBeNull();
    expect(favoriteRow).not.toBeNull();
    expect(within(regularRow!).queryByLabelText("已收藏")).not.toBeInTheDocument();
    expect(within(regularRow!).queryByText("未收藏")).not.toBeInTheDocument();
    const marker = within(favoriteRow!).getByRole("img", { name: "已收藏" });
    expect(marker).toBeInTheDocument();
    expect(marker.closest(".row-metadata")).not.toBeNull();
  });

  it("does not show a completed badge for successful file history", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));

    const completedRow = (await screen.findByText("notes.pdf")).closest("article");
    expect(completedRow).not.toBeNull();
    expect(within(completedRow!).queryByText("已完成")).not.toBeInTheDocument();

    const activeRow = screen.getByText("SyncHalo-design.zip").closest("article");
    expect(activeRow).not.toBeNull();
    fireEvent.click(within(activeRow!).getByRole("button", { name: "SyncHalo-design.zip" }));
    expect(screen.queryByText("已完成")).not.toBeInTheDocument();
  });

  it("deletes one clipboard row", async () => {
    render(<App />);
    expect(await screen.findByText("cargo test --workspace")).toBeInTheDocument();
    const deleteButtons = screen.getAllByRole("button", { name: "删除这条历史" });
    fireEvent.click(deleteButtons[0]);
    await waitFor(() => expect(screen.queryByText("cargo test --workspace")).not.toBeInTheDocument());
    expect(screen.getByText("历史已删除")).toBeInTheDocument();
  });

  it("uses the shared modal shell for clearing history", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: "清空" }));
    const dialog = screen.getByRole("dialog", { name: "清空粘贴板历史？" });
    expect(dialog).toHaveClass("dialog-backdrop");
    expect(dialog.querySelector(".confirm-dialog")).toHaveClass("modal-dialog");
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(dialog).not.toBeInTheDocument();
  });

  it("shows the current device first, exposes the sync code, and narrows targets", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));

    const fileActions = document.querySelector<HTMLElement>(".files-page .page-actions");
    expect(fileActions?.firstElementChild).toHaveClass("search-field");
    expect(screen.getByPlaceholderText("搜索历史")).toBeInTheDocument();
    for (const removedFilter of ["全部", "已发送", "已接收", "进行中", "失败"]) {
      expect(screen.queryByRole("button", { name: removedFilter })).not.toBeInTheDocument();
    }

    const deviceRows = document.querySelectorAll(".sync-device-row");
    expect(deviceRows[0]).toHaveTextContent("Jason 的 MacBook Air");
    expect(deviceRows[0]).toHaveTextContent("本机");
    fireEvent.click(screen.getByRole("button", { name: "显示同步码" }));
    const syncCodeDialog = await screen.findByRole("dialog", { name: "连接另一台设备" });
    expect(syncCodeDialog).toHaveClass("dialog-backdrop--contained", "dialog-backdrop--strong");
    expect(syncCodeDialog.querySelector(".sync-code-dialog")).toHaveClass("modal-dialog");
    expect(await screen.findByText("482 913")).toBeInTheDocument();

    expect(screen.getByText("全部在线设备")).toBeInTheDocument();
    expect(screen.getByText("未指定目标，将同步到全部 2 台在线设备")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Studio Ubuntu/ }));
    expect(screen.getByText("1 个指定目标")).toBeInTheDocument();
    expect(screen.getByText("将同步到 1 台指定设备")).toBeInTheDocument();
    expect(screen.queryByText(/01 ·|02 ·|03 ·/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /粘贴并同步/ })).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "v", metaKey: true });
    expect(await screen.findByText("clipboard-file.zip")).toBeInTheDocument();
  });

  it("paginates file history 100 items per page and returns to the top", async () => {
    const { container } = render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));
    expect(await screen.findByText(/第 1 \/ 3 页 · 共 \d+ 条 · 每页 100 条/)).toBeInTheDocument();

    const scroller = container.querySelector<HTMLElement>(".files-page .page-scroll");
    expect(scroller).not.toBeNull();
    scroller!.scrollTop = 720;
    fireEvent.click(screen.getByRole("button", { name: "文件历史下一页" }));
    expect(scroller!.scrollTop).toBe(0);
    expect(await screen.findByText(/第 2 \/ 3 页 · 共 \d+ 条 · 每页 100 条/)).toBeInTheDocument();
  });

  it("uses the merged drop zone for choosing and dropping files", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));

    expect(screen.queryByRole("button", { name: "选择文件" })).not.toBeInTheDocument();
    const dropZone = screen.getByRole("button", { name: "拖入文件或选择文件" });
    fireEvent.click(dropZone);
    expect(await screen.findByText("release-arm64.deb")).toBeInTheDocument();

    fireEvent.drop(dropZone, {
      dataTransfer: { files: [new File(["payload"], "dragged-report.pdf")] },
    });
    expect(await screen.findByText("dragged-report.pdf")).toBeInTheDocument();
  });

  it("sends to every online device when no sync target is selected", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));

    expect(screen.getByText("全部在线设备")).toBeInTheDocument();
    expect(screen.getByText("未指定目标，将同步到全部 2 台在线设备")).toBeInTheDocument();
    const dropZone = screen.getByRole("button", { name: "拖入文件或选择文件" });
    fireEvent.drop(dropZone, {
      dataTransfer: { files: [new File(["payload"], "all-online-default.pdf")] },
    });

    const row = (await screen.findByText("all-online-default.pdf")).closest("article");
    expect(row).not.toBeNull();
    expect(row).toHaveTextContent("发送到 Studio Ubuntu、Desk Pi");
    expect(row).not.toHaveTextContent("Office Ubuntu");
    expect(screen.queryByText("等待设备上线")).not.toBeInTheDocument();
  });

  it("does not allow an offline device to be selected", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));
    const offlineDevice = screen.getByRole("button", { name: /Office Ubuntu/ });

    expect(offlineDevice).toBeDisabled();
    fireEvent.click(offlineDevice);
    expect(screen.getByText("全部在线设备")).toBeInTheDocument();
    expect(screen.getByText("未指定目标，将同步到全部 2 台在线设备")).toBeInTheDocument();
  });

  it("filters favorite file history and can resync an earlier file", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));

    fireEvent.click(screen.getByRole("button", { name: "只看收藏文件" }));
    await waitFor(() => expect(screen.queryByText("SyncHalo-design.zip")).not.toBeInTheDocument());
    expect(screen.getByText("notes.pdf")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "显示全部文件历史" }));
    const row = (await screen.findByText("SyncHalo-design.zip")).closest("article");
    expect(row).not.toBeNull();
    fireEvent.click(within(row!).getByRole("button", { name: "再次同步" }));
    await waitFor(() => expect(screen.getAllByText("SyncHalo-design.zip").length).toBeGreaterThan(1));
  });

  it("clears finished file history while preserving favorites and active tasks", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "粘贴板历史" });
    fireEvent.click(screen.getByRole("button", { name: /同步文件/ }));
    expect(await screen.findByText("dataset.tar")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "清空" }));
    const dialog = screen.getByRole("dialog", { name: "清空同步记录？" });
    expect(dialog).toHaveTextContent("收藏记录和正在进行的任务会保留");
    fireEvent.click(within(dialog).getByRole("button", { name: "清空记录" }));

    await waitFor(() => expect(screen.queryByText("dataset.tar")).not.toBeInTheDocument());
    expect(screen.getByText("notes.pdf")).toBeInTheDocument();
    expect(screen.getAllByText("SyncHalo-design.zip").length).toBeGreaterThan(0);
    expect(screen.getByText(/已清除 \d+ 条同步记录/)).toBeInTheDocument();
  });
});
