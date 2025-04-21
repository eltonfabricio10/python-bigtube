# -*- coding: utf-8 -*-
"""
Implementação da janela de histórico de mensagens
"""
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw


class HistoryWindow(Adw.ApplicationWindow):
    def __init__(self, app, messages):
        super().__init__(application=app)
        self.set_title("Histórico de Mensagens")
        self.set_default_size(400, 300)
        self.set_resizable(True)

        self.messages = messages  # Guarda referência para mensagens

        # Caixa principal
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(12)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)

        # Área de rolagem
        self.scrolled = Gtk.ScrolledWindow()
        self.scrolled.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        self.scrolled.set_vexpand(True)

        # Lista de mensagens
        self.listbox = Gtk.ListBox()
        self.listbox.set_selection_mode(Gtk.SelectionMode.NONE)
        self.scrolled.set_child(self.listbox)
        main_box.append(self.scrolled)

        # Espaço flexível para empurrar botões para baixo
        spacer = Gtk.Box()
        spacer.set_vexpand(True)
        main_box.append(spacer)

        # Caixa de botões no canto inferior direito
        button_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        button_box.set_halign(Gtk.Align.END)

        # Botão "Limpar Histórico"
        clear_button = Gtk.Button(label="Limpar")
        clear_button.connect("clicked", self._on_clear_clicked)
        button_box.append(clear_button)

        # Botão "Fechar"
        close_button = Gtk.Button(label="Fechar")
        close_button.connect("clicked", lambda _: self.close())
        close_button.add_css_class("suggested-action")
        button_box.append(close_button)

        self._populate_messages(messages, clear_button)

        main_box.append(button_box)
        self.set_content(main_box)

    def _populate_messages(self, messages, button):
        # Limpar as linhas da ListBox manualmente
        self.listbox.remove_all()

        if messages:
            history = messages[:]
        else:
            history = ["Nenhum histórico de download!"]
            button.set_sensitive(False)

        for msg in history:
            row = self._create_message_row(msg)
            self.listbox.append(row)

    def _create_message_row(self, message):
        label = Gtk.Label(label=message)
        label.set_wrap(True)
        label.set_xalign(0)
        label.set_margin_top(6)
        label.set_margin_bottom(6)
        label.set_margin_start(12)
        label.set_margin_end(12)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(label)

        row = Gtk.ListBoxRow()
        row.set_child(box)
        return row

    def _on_clear_clicked(self, button):
        self.messages.clear()
        self._populate_messages(self.messages)
