# ruff: noqa: E402
from gi.repository import Adw

from ...core.config import ConfigManager
from ...core.enums import ThemeColor, ThemeMode


class ThemeSettingsController:
    def __init__(self, window, widgets_map):
        self.window = window
        self.widgets_map = widgets_map
        self._setup_bindings()

    def _setup_bindings(self):
        w = self.widgets_map
        if "row_theme" in w:
            w["row_theme"].connect("notify::selected", self._on_theme_changed)
        if "row_theme_color" in w:
            w["row_theme_color"].connect("notify::selected", self._on_theme_color_changed)

    def _on_theme_changed(self, row, param):
        idx = row.get_selected()
        mode = ThemeMode.SYSTEM
        if idx == 1:
            mode = ThemeMode.LIGHT
        elif idx == 2:
            mode = ThemeMode.DARK

        ConfigManager.set("theme_mode", mode)

        if hasattr(self.window, "apply_theme"):
            curr_color = ConfigManager.get("theme_color")
            self.window.apply_theme(mode, curr_color)
        else:
            manager = Adw.StyleManager.get_default()
            if mode == ThemeMode.SYSTEM:
                manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
            elif mode == ThemeMode.LIGHT:
                manager.set_color_scheme(Adw.ColorScheme.FORCE_LIGHT)
            elif mode == ThemeMode.DARK:
                manager.set_color_scheme(Adw.ColorScheme.FORCE_DARK)

    def _on_theme_color_changed(self, row, param):
        idx = row.get_selected()
        c_map = {
            0: ThemeColor.DEFAULT,
            1: ThemeColor.VIOLET,
            2: ThemeColor.EMERALD,
            3: ThemeColor.SUNBURST,
            4: ThemeColor.ROSE,
            5: ThemeColor.CYAN,
            6: ThemeColor.NORDIC,
            7: ThemeColor.GRUVBOX,
            8: ThemeColor.CATPPUCCIN,
            9: ThemeColor.DRACULA,
            10: ThemeColor.TOKYO_NIGHT,
            11: ThemeColor.ROSE_PINE,
            12: ThemeColor.SOLARIZED,
            13: ThemeColor.MONOKAI,
            14: ThemeColor.CYBERPUNK,
            15: ThemeColor.BIGTUBE,
        }
        new_color = c_map.get(idx, ThemeColor.DEFAULT)
        ConfigManager.set("theme_color", new_color)

        if hasattr(self.window, "apply_theme"):
            curr_mode = ConfigManager.get("theme_mode")
            self.window.apply_theme(curr_mode, new_color)
