from pathlib import Path

from PIL import Image
from playwright.sync_api import expect, sync_playwright


ROOT = Path(__file__).resolve().parents[1]
DESKTOP_SCREENSHOT = ROOT / ".tmp-skills-hub-custom-runtime-desktop.png"
CONSTRAINED_SCREENSHOT = ROOT / ".tmp-skills-hub-custom-runtime-constrained.png"
URL = "http://127.0.0.1:5173"


MOCK_TAURI = r"""
(() => {
  const calls = [];
  const autoUpdateConfig = {
    enabled: false,
    interval_hours: 24,
    schedule_type: "interval",
    interval_value: 24,
    interval_unit: "hours",
    daily_time: "09:00",
    local_skill_count: 0,
    protected_local_skill_count: 0,
    task_registered: false,
    task_status_detail: "",
    last_run_at: null,
    last_started_at: null,
    last_finished_at: null,
    last_status: null,
    last_error: null,
    last_checked: 0,
    last_updated: 0,
    last_failed: 0,
    progress: { total: 0, succeeded: [], failed: [], running: null, pending: [] },
  };
  const invoke = async (cmd, args = {}) => {
    calls.push({ cmd, args });
    if (cmd === "plugin:app|version") return "0.9.1-custom.1";
    if (cmd === "plugin:updater|check") return null;
    if (cmd === "get_managed_skills" || cmd === "get_tags" || cmd === "get_recent_projects") return [];
    if (cmd === "get_central_repo_path") return "C:\\Users\\demo\\.skillshub-custom";
    if (cmd === "get_git_cache_cleanup_days") return 30;
    if (cmd === "get_git_cache_ttl_secs") return 300;
    if (cmd === "get_github_token") return "";
    if (cmd === "get_github_proxy_config") return { enabled: false, port: 7890, url: "", auto_detected: false };
    if (cmd === "get_auto_update_config") return autoUpdateConfig;
    if (cmd === "get_tool_config") return { disabled_builtin_tools: [], custom_tools: [] };
    if (cmd === "get_discovery_scan_settings") return { sources: [], disabled_source_keys: [] };
    if (cmd === "get_onboarding_plan") return { total_tools_scanned: 0, total_skills_found: 0, groups: [] };
    if (cmd === "get_tool_status") return { tools: [], installed: [], newly_installed: [] };
    if (cmd === "get_runtime_evidence_status") return {
      schema_version: 1,
      event_name: "runtime-evidence://event-v1",
      collector_state: "not_configured",
      last_event_at_ms: null,
      supported_event_types: ["session_started", "skill_loaded", "skill_called"],
    };
    throw new Error(`Unexpected mock IPC command: ${cmd}`);
  };
  window.__SKILLS_HUB_CUSTOM_CALLS__ = calls;
  window.__TAURI_INTERNALS__ = { invoke };
})();
"""


def assert_nonblank(path: Path) -> None:
    assert path.exists() and path.stat().st_size > 10_000, path
    with Image.open(path).convert("RGB") as image:
        assert any(low != high for low, high in image.getextrema()), path


def assert_no_overflow(page) -> None:
    sizes = page.evaluate(
        """() => ({
          viewport: window.innerWidth,
          html: document.documentElement.scrollWidth,
          body: document.body.scrollWidth
        })"""
    )
    assert sizes["html"] <= sizes["viewport"], sizes
    assert sizes["body"] <= sizes["viewport"], sizes


def run() -> None:
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1180, "height": 760})
        page.add_init_script(MOCK_TAURI)
        page.goto(URL)
        page.wait_for_load_state("networkidle")
        page.evaluate("localStorage.setItem('skills-language', 'zh')")
        page.reload()
        page.wait_for_load_state("networkidle")

        page.get_by_role("button", name="运行证据", exact=True).click()
        expect(page.get_by_role("heading", name="运行时证据")).to_be_visible()
        expect(
            page.locator(".runtime-evidence-state strong").get_by_text(
                "采集器未配置", exact=True
            )
        ).to_be_visible()
        expect(page.get_by_text("已加载和最近调用状态仍为未知。", exact=True)).to_be_visible()
        expect(page.get_by_text("runtime-evidence://event-v1", exact=True)).to_be_visible()
        assert_no_overflow(page)
        page.screenshot(path=str(DESKTOP_SCREENSHOT), full_page=True)

        page.set_viewport_size({"width": 900, "height": 640})
        expect(page.get_by_role("heading", name="运行时证据")).to_be_visible()
        assert_no_overflow(page)
        page.screenshot(path=str(CONSTRAINED_SCREENSHOT), full_page=True)

        runtime_calls = page.evaluate(
            "window.__SKILLS_HUB_CUSTOM_CALLS__.filter(call => call.cmd === 'get_runtime_evidence_status').length"
        )
        assert runtime_calls in (1, 2), runtime_calls

        browser.close()

    assert_nonblank(DESKTOP_SCREENSHOT)
    assert_nonblank(CONSTRAINED_SCREENSHOT)
    print("runtime evidence UI acceptance: passed")
    print(f"desktop screenshot: {DESKTOP_SCREENSHOT}")
    print(f"constrained screenshot: {CONSTRAINED_SCREENSHOT}")


if __name__ == "__main__":
    run()
