import gi
gi.require_version('Adw', '1')
from gi.repository import Adw

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

        if is_error:
            toast = Adw.Toast.new(message)
            toast.set_timeout(5)
            toast.set_priority(Adw.ToastPriority.HIGH)
            cls._toast_widget.add_toast(toast)
        else:
            toast = Adw.Toast.new(message)
            toast.set_timeout(5)
            toast.set_priority(Adw.ToastPriority.NORMAL)
            cls._toast_widget.add_toast(toast)

    @classmethod
    def show_confirmation(cls, title: str, body: str, on_confirm_callback=None):
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
        txt_cancel = Res.get(StringKey.STATUS_CANCEL) or "Cancel"
        txt_confirm = Res.get(StringKey.STATUS_CONFIRM) or "Confirm"

        dialog.add_response("cancel", txt_cancel)
        dialog.add_response("confirm", txt_confirm)
        dialog.set_response_appearance("confirm", Adw.ResponseAppearance.DESTRUCTIVE)

        dialog.set_default_response("cancel")
        dialog.set_close_response("cancel")

        def _callback(dialog, result):
            response = dialog.choose_finish(result)
            if response == "confirm" and on_confirm_callback:
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
        txt_ok = Res.get(StringKey.BTN_OK)
        dialog.add_response("close", txt_ok)
        dialog.set_response_appearance("close", Adw.ResponseAppearance.DEFAULT)

        dialog.set_default_response("close")
        dialog.set_close_response("close")

        dialog.choose(cls._window, None, cls._on_dialog_response)

    @classmethod
    def show_info_dialog(cls, title: str, body: str, on_close_callback=None):
        """
        Shows a simple information modal with an OK button.
        """
        if not cls._window:
            return

        dialog = Adw.AlertDialog(
            heading=title,
            body=body
        )

        txt_ok = Res.get(StringKey.STATUS_CONFIRM) or "OK"
        dialog.add_response("ok", txt_ok)
        dialog.set_default_response("ok")
        dialog.set_close_response("ok")

        def _callback(dialog, result):
            dialog.choose_finish(result)
            if on_close_callback:
                on_close_callback()

        dialog.choose(cls._window, None, _callback)

    @classmethod
    def show_custom_dialog(cls, title: str, body: str, responses: dict, on_response_callback=None, destructive_id=None):
        """
        Shows a native Adwaita Alert Dialog with multiple custom response buttons.
        responses: dict of {id: label}
        """
        if not cls._window:
            print("[UI Error] MessageManager missing parent window for dialog.")
            return

        dialog = Adw.AlertDialog(heading=title, body=body)

        for resp_id, label in responses.items():
            dialog.add_response(resp_id, label)
            if resp_id == destructive_id:
                dialog.set_response_appearance(resp_id, Adw.ResponseAppearance.DESTRUCTIVE)

        def _callback(dialog, result):
            response = dialog.choose_finish(result)
            if on_response_callback:
                on_response_callback(response)

        dialog.choose(cls._window, None, _callback)

    @classmethod
    def _on_dialog_response(cls, dialog, result):
        """Finalizes the dialog lifecycle."""
        dialog.choose_finish(result)
