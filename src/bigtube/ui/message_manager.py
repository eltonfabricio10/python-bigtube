import gi
from gi.repository import GLib, Adw

# Internal Imports
from ..core.locales import ResourceManager as Res, StringKey


class MessageManager:
    """
    Centralized service for displaying UI feedback.
    Handles:
    1. Transient Toasts (Top Overlay)
    2. Modal Dialogs (Confirmation/Error)
    """

    _toast_widget = None
    _window = None
    _timer_id = None

    @classmethod
    def init(cls, toast_widget, window=None):
        """
        Injects the visual dependencies (The Toast Widget and Main Window).
        """
        cls._toast_widget = toast_widget
        cls._window = window

    @classmethod
    def show(cls, message: str, is_error: bool = False):
        """
        Displays a transient notification at the top of the app.
        Replaces 'show' to match calls from other controllers.
        """
        if not cls._toast_widget:
            print("[UI Error] MessageManager not initialized.")
            return

        # 1. Cancel previous hide timer if active (Reset countdown)
        if cls._timer_id is not None:
            GLib.source_remove(cls._timer_id)
            cls._timer_id = None

        # 2. Update Visuals
        cls._toast_widget.update_style(message, is_error)
        cls._toast_widget.animate_in()

        # 3. Schedule Hide (5 seconds)
        # We store the ID to cancel it if a new message arrives quickly
        cls._timer_id = GLib.timeout_add_seconds(5, cls._on_timeout_hide)

    @classmethod
    def _on_timeout_hide(cls):
        """Helper to hide toast and clear timer reference."""
        if cls._toast_widget:
            cls._toast_widget.animate_out()
        cls._timer_id = None
        return False  # Stop GLib timer

    @classmethod
    def show_confirmation(cls, title: str, body: str, on_confirm_callback):
        """
        Shows a native Adwaita Alert Dialog for confirmation.
        """
        if not cls._window:
            print("[UI Error] MessageManager missing parent window for dialog.")
            return

        dialog = Adw.AlertDialog(heading=title, body=body)

        # Responses
        # Note: You should verify these keys exist in your locales.py
        # or use literal strings if you prefer for now.
        txt_cancel = Res.get(StringKey.STATUS_CANCELLED) or "Cancel"
        txt_confirm = "Confirm"

        dialog.add_response("cancel", txt_cancel)
        dialog.add_response("confirm", txt_confirm)
        dialog.set_response_appearance("confirm", Adw.ResponseAppearance.DESTRUCTIVE)

        dialog.set_default_response("cancel")
        dialog.set_close_response("cancel")

        def _callback(dialog, result):
            response = dialog.choose_finish(result)
            if response == "confirm":
                on_confirm_callback()

        dialog.choose(cls._window, None, _callback)

    @classmethod
    def show_error_dialog(cls, title: str, body: str):
        """
        Shows a critical error modal.
        """
        if not cls._window:
            return

        dialog = Adw.AlertDialog(
            heading=title,
            body=body
        )

        # "Close" or "OK" button
        dialog.add_response("close", "OK")
        dialog.set_response_appearance("close", Adw.ResponseAppearance.DEFAULT)

        dialog.set_default_response("close")
        dialog.set_close_response("close")

        dialog.choose(cls._window, None, cls._on_dialog_response)

    @classmethod
    def _on_dialog_response(cls, dialog, result):
        """Finalizes the dialog lifecycle."""
        dialog.choose_finish(result)
