import re
from pathlib import Path

from playwright.sync_api import sync_playwright


BASE_URL = "http://127.0.0.1:1420"
ARTIFACTS = Path(__file__).resolve().parents[2] / "artifacts" / "ui"


def main() -> None:
    console_errors: list[str] = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 800}, device_scale_factor=1)
        page.on(
            "console",
            lambda message: console_errors.append(message.text)
            if message.type == "error"
            else None,
        )
        page.goto(BASE_URL)
        page.wait_for_load_state("networkidle")

        page.get_by_role("heading", name="粘贴板历史").wait_for()
        assert page.get_by_role("navigation", name="主导航").is_visible()
        assert page.locator(".clipboard-page .page-header").get_by_role(
            "button", name="暂停同步"
        ).count() == 0
        first_row = page.locator(".clipboard-row").first
        second_row = page.locator(".clipboard-row").nth(1)
        row_actions = first_row.locator(".row-actions")
        assert row_actions.evaluate("element => getComputedStyle(element).opacity") == "0"
        assert first_row.locator(".favorite-marker svg").count() == 0
        assert page.get_by_text("未收藏", exact=True).count() == 0
        assert second_row.get_by_role("img", name="已收藏").is_visible()
        assert second_row.locator(".favorite-marker svg").count() == 1
        page.screenshot(path=ARTIFACTS / "clipboard-light.png", full_page=True)
        first_row.locator(".clipboard-copy > p").click()
        assert page.get_by_text("已复制", exact=True).count() == 0

        first_row.hover()
        page.wait_for_timeout(140)
        assert row_actions.evaluate("element => getComputedStyle(element).opacity") == "1"
        assert first_row.get_by_role("button", name="复制这条历史").is_visible()
        assert first_row.get_by_role("button", name="查看完整内容").is_visible()
        assert first_row.get_by_role("button", name="收藏").is_visible()
        assert first_row.get_by_role("button", name="删除这条历史").is_visible()
        first_row.get_by_role("button", name="查看完整内容").click()
        preview = page.get_by_role("dialog", name="完整内容")
        assert preview.get_by_label("完整剪贴板内容").input_value() == "cargo test --workspace"
        page.screenshot(path=ARTIFACTS / "clipboard-preview.png", full_page=True)
        preview.get_by_role("button", name="关闭完整内容").click()
        page.screenshot(path=ARTIFACTS / "clipboard-hover-actions.png", full_page=True)

        pagination = page.get_by_role("navigation", name="粘贴板历史分页")
        pagination.scroll_into_view_if_needed()
        page.get_by_text("第 1 / 3 页 · 共 205 条 · 每页 100 条").wait_for()
        assert pagination.evaluate("element => getComputedStyle(element).borderTopStyle") == "none"
        assert page.locator(".clipboard-row").last.evaluate(
            "element => getComputedStyle(element).borderBottomStyle"
        ) == "none"
        page.screenshot(path=ARTIFACTS / "clipboard-pagination.png", full_page=True)
        page.get_by_role("button", name="下一页").click()
        page.get_by_text("第 2 / 3 页 · 共 205 条 · 每页 100 条").wait_for()
        assert page.locator(".clipboard-page .page-scroll").evaluate("element => element.scrollTop") == 0
        page.get_by_text("历史记录 #101 · SyncHalo 分页数据").wait_for()
        page.get_by_role("button", name="只看收藏").click()
        page.get_by_text("共 5 条", exact=True).wait_for()
        page.screenshot(path=ARTIFACTS / "clipboard-favorites.png", full_page=True)
        page.get_by_role("button", name="显示全部历史").click()
        page.get_by_text("第 1 / 3 页 · 共 205 条 · 每页 100 条").wait_for()

        page.get_by_role("button", name="同步文件 ⌘2").click()
        page.get_by_role("heading", name="同步文件").wait_for()
        file_actions = page.locator(".files-page .page-actions")
        assert "search-field" in file_actions.locator(":scope > *").first.get_attribute("class")
        assert page.locator(".files-page .page-header").get_by_placeholder("搜索历史").is_visible()
        assert page.locator(".files-page .segmented-control").count() == 0
        assert page.get_by_role("heading", name="我的设备").is_visible()
        assert page.get_by_text("已完成", exact=True).count() == 0
        first_device = page.locator(".sync-device-row").first
        assert first_device.get_by_text("Jason 的 MacBook Air").is_visible()
        assert first_device.get_by_text("本机", exact=True).is_visible()
        assert page.get_by_text("把文件拖入或者直接粘贴文件").is_visible()
        assert page.get_by_text(re.compile(r"^(01|02|03) ·")).count() == 0
        assert page.get_by_role("button", name="粘贴并同步").count() == 0
        assert page.get_by_role("button", name="选择文件", exact=True).count() == 0
        drop_zone = page.get_by_role("button", name="拖入文件或选择文件")
        assert drop_zone.is_visible()
        assert page.get_by_role("button", name="只看收藏文件").is_visible()
        page.screenshot(path=ARTIFACTS / "files-light.png", full_page=True)

        page.get_by_role("button", name="显示同步码").click()
        sync_code_dialog = page.get_by_role("dialog", name="连接另一台设备")
        sync_code_dialog.wait_for()
        sync_code_backdrop = sync_code_dialog
        assert sync_code_backdrop.evaluate(
            "element => getComputedStyle(element).backgroundColor"
        ) == "rgba(0, 0, 0, 0.7)"
        content_box = page.locator(".content-pane").bounding_box()
        dialog_box = page.locator(".sync-code-dialog").bounding_box()
        assert content_box is not None and dialog_box is not None
        assert abs((dialog_box["x"] + dialog_box["width"] / 2) - (content_box["x"] + content_box["width"] / 2)) < 2
        assert abs((dialog_box["y"] + dialog_box["height"] / 2) - (content_box["y"] + content_box["height"] / 2)) < 2
        page.get_by_text("482 913").wait_for()
        page.screenshot(path=ARTIFACTS / "files-sync-code.png", full_page=True)
        page.get_by_role("button", name="关闭连接另一台设备").click()

        drop_zone.click()
        page.get_by_text("release-arm64.deb").wait_for()
        page.keyboard.press("Meta+V")
        page.get_by_text("clipboard-file.zip").wait_for()
        page.get_by_role("button", name="只看收藏文件").click()
        page.get_by_role("button", name="显示全部文件历史").wait_for()
        page.get_by_text("notes.pdf").wait_for()
        assert page.get_by_text("SyncHalo-design.zip").count() == 0
        page.screenshot(path=ARTIFACTS / "files-favorites.png", full_page=True)
        page.get_by_role("button", name="显示全部文件历史").click()

        original_row = page.locator(".transfer-row").filter(has_text="SyncHalo-design.zip").first
        original_row.get_by_role("button", name="再次同步").click()
        page.wait_for_function(
            "() => [...document.querySelectorAll('.transfer-row')].filter((row) => row.textContent?.includes('SyncHalo-design.zip')).length > 1"
        )

        file_pagination = page.get_by_role("navigation", name="文件历史分页")
        file_pagination.scroll_into_view_if_needed()
        assert "每页 100 条" in file_pagination.locator(".pagination-summary").inner_text()
        page.screenshot(path=ARTIFACTS / "files-pagination.png", full_page=True)
        page.get_by_role("button", name="文件历史下一页").click()
        page.get_by_text(re.compile(r"^第 2 / 3 页 · 共 \d+ 条 · 每页 100 条$")).wait_for()
        assert page.locator(".files-page .page-scroll").evaluate("element => element.scrollTop") == 0

        page.get_by_role("button", name="设置 ⌘,").click()
        page.get_by_role("heading", name="设置").wait_for()
        assert page.get_by_text("我的设备").is_visible()
        assert page.get_by_text("同步与历史").is_visible()
        assert page.get_by_text("粘贴板与历史").count() == 0
        assert page.get_by_text("自动同步粘贴板").count() == 0
        assert page.get_by_text("平台能力").count() == 0
        assert page.get_by_text(re.compile(r"Protocol v1", re.IGNORECASE)).count() == 0
        assert not page.get_by_role("switch", name="删除同步").is_checked()
        assert not page.get_by_role("switch", name="收藏同步").is_checked()
        assert page.get_by_text("SyncHalo 0.1.1", exact=True).is_visible()
        page.get_by_text("482 913").wait_for()
        assert page.locator(".section-intro p").count() == 0
        assert page.get_by_text("传输完成与错误通知").count() == 0
        assert page.get_by_label("加入另一台设备").count() == 0
        assert page.locator(".settings-section").first.evaluate(
            "element => getComputedStyle(element).gridTemplateColumns.split(' ')[0]"
        ) == "128px"
        page.screenshot(path=ARTIFACTS / "settings-light.png", full_page=True)
        page.get_by_role("button", name="加入", exact=True).click()
        join_dialog = page.get_by_role("dialog", name="加入另一台设备")
        join_dialog.wait_for()
        page.get_by_label("输入一次性同步码").fill("482913")
        page.screenshot(path=ARTIFACTS / "settings-join-dialog.png", full_page=True)
        page.get_by_role("button", name="取消", exact=True).click()

        page.emulate_media(color_scheme="dark")
        page.wait_for_timeout(150)
        page.screenshot(path=ARTIFACTS / "settings-dark.png", full_page=True)

        page.set_viewport_size({"width": 860, "height": 560})
        page.emulate_media(color_scheme="light")
        assert page.locator(".settings-section").first.evaluate(
            "element => getComputedStyle(element).gridTemplateColumns.split(' ')[0]"
        ) == "112px"
        page.screenshot(path=ARTIFACTS / "settings-minimum.png", full_page=True)
        page.locator(".nav-item").filter(has_text="同步文件").click()
        page.wait_for_timeout(100)
        page.screenshot(path=ARTIFACTS / "files-minimum.png", full_page=True)
        page.locator(".nav-item").filter(has_text="粘贴板").click()
        page.wait_for_timeout(100)
        dimensions = page.evaluate(
            "() => ({ width: document.documentElement.scrollWidth, viewport: window.innerWidth })"
        )
        assert dimensions["width"] <= dimensions["viewport"]
        page.screenshot(path=ARTIFACTS / "clipboard-minimum.png", full_page=True)

        browser.close()

    if console_errors:
        raise AssertionError(f"browser console errors: {console_errors}")


if __name__ == "__main__":
    main()
