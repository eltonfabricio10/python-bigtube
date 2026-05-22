# ruff: noqa: E402
import threading

from gi.repository import GLib

from ...core.locales import ResourceManager as Res
from ...core.locales import StringKey
from ...core.logger import get_logger
from ...core.updater import Updater
from ...ui.message_manager import MessageManager

logger = get_logger(__name__)


class SystemSettingsController:
    def __init__(self, btn_update, row_version):
        self.btn_update = btn_update
        self.row_version = row_version
        self._setup_bindings()

    def _setup_bindings(self):
        self.btn_update.connect("clicked", self.on_check_update_clicked)

    def on_check_update_clicked(self, btn):
        self.btn_update.set_sensitive(False)
        threading.Thread(target=self._run_update_process, daemon=True).start()

    def _run_update_process(self):
        try:
            ok_bin, new_ver = Updater.update_yt_dlp()
            ok_deno = Updater.update_deno()
            GLib.idle_add(self._on_update_finished, ok_bin, ok_deno, new_ver)
        except Exception as e:
            logger.error(f"Update Exception: {e}")
            GLib.idle_add(self._on_update_error, str(e))

    def _on_update_finished(self, ok_bin, ok_deno, new_ver):
        self.btn_update.set_sensitive(True)
        if ok_bin and ok_deno:
            self.row_version.set_subtitle(f"yt-dlp v{new_ver}")
            MessageManager.show(Res.get(StringKey.MSG_UPDATE_SUCCESS), is_error=False)
        else:
            if ok_deno:
                MessageManager.show(Res.get(StringKey.MSG_UPDATE_DENO_ONLY), is_error=True)
            else:
                MessageManager.show(Res.get(StringKey.MSG_UPDATE_FAILED), is_error=True)

    def _on_update_error(self, error_msg):
        self.btn_update.set_sensitive(True)
        prefix = Res.get(StringKey.MSG_GENERIC_ERROR_PREFIX)
        MessageManager.show(f"{prefix} {error_msg}", is_error=True)

    def async_load_version(self):
        v_prefix = Res.get(StringKey.LBL_VERSION_PREFIX)
        ver = Updater.get_local_version() or "v?"
        GLib.idle_add(self.row_version.set_subtitle, f"{v_prefix}{ver}")
