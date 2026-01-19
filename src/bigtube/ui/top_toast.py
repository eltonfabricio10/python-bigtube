import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk


class TopToast(Gtk.Revealer):
    """
    Reusable UI component for top-screen notifications.
    Encapsulates visual logic (Box, Label, CSS, Animation).
    """

    def __init__(self):
        super().__init__()

        # Revealer Configuration (Root Container)
        self.set_valign(Gtk.Align.START)   # Stick to Top
        self.set_halign(Gtk.Align.CENTER)  # Center Horizontally
        self.set_transition_type(Gtk.RevealerTransitionType.SLIDE_DOWN)
        self.set_can_target(False)

        # Visual Container (The Pill)
        self._box = Gtk.Box()
        self._box.add_css_class("top-toast")

        # Text Label
        self._label = Gtk.Label()
        self._label.set_justify(2)
        self._label.set_wrap(False)
        self._box.append(self._label)

        # Set the box as the child of the Revealer
        self.set_child(self._box)

    def update_style(self, text: str, is_error: bool):
        """
        Updates the label text and toggles CSS classes for state.
        """
        self._label.set_label(text)

        if is_error:
            self._box.add_css_class("error")
            self._box.remove_css_class("success")
        else:
            self._box.remove_css_class("error")
            self._box.add_css_class("success")

    def animate_in(self):
        """Slides the toast down into view."""
        self.set_reveal_child(True)

    def animate_out(self):
        """
        Slides the toast up out of view.
        Returns False to ensure GLib.timeout_add stops repeating if used there.
        """
        self.set_reveal_child(False)
        return False
