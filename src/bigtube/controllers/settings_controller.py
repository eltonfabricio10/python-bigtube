import threading
import os
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gio, GLib

# Internal Imports
from ..core.config import ConfigManager
from ..core.updater import Updater
from ..core.locales import ResourceManager as Res, StringKey
from ..ui.message_manager import MessageManager


class SettingsController:
    """
    Manages logic for the Settings/Preferences page.
    Handles Folder Selection and Software Updates.
    """

    def __init__(self, row_folder, btn_pick, row_version, btn_update, window_parent):
        """
        Initializes the controller with widget references from MainWindow.
        """
        self.row_folder = row_folder
        self.btn_pick = btn_pick
        self.row_version = row_version
        self.btn_update = btn_update
        self.window = window_parent

        # 1. Load Initial Data
        self._load_initial_state()

        # 2. Connect Signals
        self.btn_pick.connect("clicked", self.on_pick_folder_clicked)
        self.btn_update.connect("clicked", self.on_check_update_clicked)

    def _load_initial_state(self):
        """Populates the UI with current config values."""
        # Set Download Path subtitle
        saved_path = ConfigManager.get_download_path()
        self.row_folder.set_subtitle(saved_path)

        # Set Version (Async to avoid lag on startup)
        threading.Thread(target=self._async_load_version, daemon=True).start()

    def _async_load_version(self):
        """Fetches binary version in background."""
        ver = Updater.get_local_version() or "Unknown"
        # Always update UI on Main Thread
        GLib.idle_add(self.row_version.set_subtitle, f"yt-dlp v{ver}")

    # =========================================================================
    # FOLDER SELECTION LOGIC
    # =========================================================================

    def on_pick_folder_clicked(self, btn):
        """Opens GTK4 FileDialog to select download directory."""
        dialog = Gtk.FileDialog()
        dialog.set_title(Res.get(StringKey.PREFS_FOLDER_LABEL))

        # Try to set initial folder to current config
        current_path = ConfigManager.get_download_path()
        try:
            if os.path.exists(current_path):
                f = Gio.File.new_for_path(current_path)
                dialog.set_initial_folder(f)
        except Exception as e:
            print(f"[Settings] Warning setting initial folder: {e}")

        # Open Modal
        dialog.select_folder(self.window, None, self._on_folder_selected)

    def _on_folder_selected(self, dialog, result):
        """Callback for folder selection."""
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                new_path = folder.get_path()

                # 1. Update Config (Auto-saves)
                ConfigManager.set("download_path", new_path)

                # 2. Update UI
                self.row_folder.set_subtitle(new_path)
                print(f"[Settings] New download path: {new_path}")

        except Exception as e:
            print(f"[Settings] Error selecting folder: {e}")
            # Optional: Show error toast
            MessageManager.show("Failed to select folder", is_error=True)

    # =========================================================================
    # UPDATE LOGIC
    # =========================================================================

    def on_check_update_clicked(self, btn):
        """Triggers the update process in a background thread."""
        # 1. Lock UI
        self.btn_update.set_sensitive(False)
        # "Pending..."
        self.btn_update.set_label(Res.get(StringKey.STATUS_PENDING))

        # 2. Run Thread
        threading.Thread(target=self._run_update_process, daemon=True).start()

    def _run_update_process(self):
        """Worker thread: Downloads and installs updates."""
        try:
            # Update yt-dlp
            ok_bin, new_ver = Updater.update_yt_dlp()
            # Update Deno
            ok_deno = Updater.update_deno()

            # Schedule UI Update
            GLib.idle_add(
                self._on_update_finished,
                ok_bin,
                ok_deno,
                new_ver
            )

        except Exception as e:
            print(f"[Settings] Update Exception: {e}")
            GLib.idle_add(self._on_update_error, str(e))

    def _on_update_finished(self, ok_bin, ok_deno, new_ver):
        """Called on Main Thread when update completes."""
        self.btn_update.set_sensitive(True)
        # Reset label (Assuming the original label was "Check for Updates")
        # You might want a specific StringKey for this button label
        self.btn_update.set_label("Check Updates")

        if ok_bin:
            self.row_version.set_subtitle(f"yt-dlp v{new_ver}")
            MessageManager.show("Components updated successfully! ✅", is_error=False)
        else:
            # If yt-dlp failed but maybe Deno worked
            if ok_deno:
                MessageManager.show("Deno updated, but yt-dlp failed.", is_error=True)
            else:
                MessageManager.show("Update check failed.", is_error=True)

    def _on_update_error(self, error_msg):
        """Called on Main Thread if critical error occurs."""
        self.btn_update.set_sensitive(True)
        self.btn_update.set_label("Retry Update")
        MessageManager.show(f"Error: {error_msg}", is_error=True)
