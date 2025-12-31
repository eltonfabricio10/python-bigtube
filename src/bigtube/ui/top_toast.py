import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk


class TopToast(Gtk.Revealer):
    """
    Componente reutilizável de notificação no topo.
    Encapsula toda a lógica visual (Box, Label, CSS, Animação).
    """
    def __init__(self):
        super().__init__()

        # Configuração do Revealer (Container Raiz)
        self.set_valign(Gtk.Align.START)   # Grudado no TOPO
        self.set_halign(Gtk.Align.CENTER)  # Centralizado horizontalmente
        self.set_transition_type(Gtk.RevealerTransitionType.SLIDE_DOWN)
        self.set_can_target(False)  # Deixa clicar quando invisível

        # Container Visual
        self._box = Gtk.Box()
        self._box.add_css_class("top-toast")

        # Label de Texto
        self._label = Gtk.Label()
        self._box.append(self._label)

        # Define o filho do Revealer
        self.set_child(self._box)

    def update_style(self, text, is_error):
        """Atualiza texto e cor"""
        self._label.set_label(text)

        if is_error:
            self._box.add_css_class("error")
        else:
            self._box.remove_css_class("error")

    def animate_in(self):
        self.set_reveal_child(True)

    def animate_out(self):
        self.set_reveal_child(False)
        return False
