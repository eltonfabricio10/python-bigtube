from pathlib import Path

from bigtube.controllers.settings_controller import SettingsController


def test_browser_command_exists_ignores_browser_tweak_wrappers(monkeypatch, tmp_path):
    wrapper = tmp_path / "firefox"
    wrapper.write_text("#!/bin/sh\n# browser-tweaks wrapper\n", encoding="utf-8")
    wrapper.chmod(0o755)

    controller = SettingsController.__new__(SettingsController)

    monkeypatch.setattr("os.get_exec_path", lambda: [str(tmp_path)])
    monkeypatch.setattr(
        "shutil.which",
        lambda command, path=None: str(Path(path) / command),
    )

    assert controller._browser_command_exists("firefox") is False


def test_browser_command_exists_accepts_real_executable(monkeypatch, tmp_path):
    browser = tmp_path / "firefox"
    browser.write_bytes(b"\x7fELF real browser")
    browser.chmod(0o755)

    controller = SettingsController.__new__(SettingsController)

    monkeypatch.setattr("os.get_exec_path", lambda: [str(tmp_path)])
    monkeypatch.setattr(
        "shutil.which",
        lambda command, path=None: str(Path(path) / command),
    )

    assert controller._browser_command_exists("firefox") is True


def test_available_cookie_browsers_are_cached(monkeypatch):
    controller = SettingsController.__new__(SettingsController)
    calls = []

    def fake_exists(command):
        calls.append(command)
        return command == "firefox"

    monkeypatch.setattr(controller, "_browser_command_exists", fake_exists)

    first = controller._get_available_cookie_browsers()
    second = controller._get_available_cookie_browsers()

    assert first == second
    assert ("firefox", "Firefox") in first
    assert calls == [
        "firefox",
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "brave",
        "brave-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
        "vivaldi",
        "vivaldi-stable",
        "opera",
    ]
