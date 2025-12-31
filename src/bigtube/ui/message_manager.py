import gi
gi.require_version('Adw', '1')
from gi.repository import GLib, Adw


class MessageManager:
    _toast_widget = None
    _window = None

    @classmethod
    def init(cls, toast_widget, window=None):
        """
        Recebe a instância pronta do widget TopToast.
        """
        cls._toast_widget = toast_widget
        cls._window = window

    @classmethod
    def show(cls, message, is_error=False):
        """Exibe a mensagem no topo."""
        if not cls._toast_widget:
            print("[UI Error] TopToast não foi inicializado!")
            return

        # 1. Atualiza o visual do widget
        cls._toast_widget.update_style(message, is_error)

        # 2. Mostra
        cls._toast_widget.animate_in()

        # 3. Agenda para esconder (3s)
        # Passamos o método animate_out do widget para o GLib
        if cls._toast_widget:
            GLib.timeout_add_seconds(3, cls._toast_widget.animate_out)

    @classmethod
    def show_confirmation(cls, title, body, on_confirm_callback):
        if not cls._window:
            return

        dialog = Adw.AlertDialog(heading=title, body=body)

        # Botão Cancelar (Esquerda)
        dialog.add_response("cancel", "Não")
        dialog.set_response_appearance("cancel", Adw.ResponseAppearance.DESTRUCTIVE)

        # Botão Confirmar (Direita - Destrutivo ou Sugerido)
        dialog.add_response("confirm", "Sim")
        dialog.set_response_appearance("confirm", Adw.ResponseAppearance.SUGGESTED)

        dialog.set_default_response("confirm")
        dialog.set_close_response("cancel")

        def _callback(dialog, result):
            response = dialog.choose_finish(result)
            if response == "confirm":
                on_confirm_callback()

        dialog.choose(cls._window, None, _callback)

    @classmethod
    def show_error_dialog(cls, title, body):
        """Mantém a função de diálogo crítico se precisar"""
        if not cls._window:
            return

        dialog = Adw.AlertDialog(
            heading=title,
            body=body
        )

        dialog.add_response("close", "Got it!")
        dialog.set_response_appearance("close", Adw.ResponseAppearance.DESTRUCTIVE)
        dialog.set_default_response("close")
        dialog.set_close_response("close")
        dialog.choose(cls._window, None, cls._on_dialog_response)

    @classmethod
    def _on_dialog_response(cls, dialog, result):
        """
        Callback chamado quando o usuário clica em um botão.
        Necessário para finalizar o ciclo do AlertDialog corretamente.
        """

        dialog.choose_finish(result)
