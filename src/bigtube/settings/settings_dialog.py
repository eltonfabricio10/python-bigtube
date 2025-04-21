# -*- coding: utf-8 -*-
"""
Diálogo de configurações do aplicativo
"""
import os
import gi
import json
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GLib, Gio


class SettingsDialog(Adw.PreferencesDialog):
    """Diálogo de configurações do BigTube"""

    def __init__(self, parent, settings):
        super().__init__(title="Configurações")
        self.settings = settings
        self.win = parent

        # Configurar páginas
        self.setup_general_page()
        self.setup_appearance_page()
        self.setup_advanced_page()

    def setup_general_page(self):
        """Configura a página de configurações gerais"""
        page = Adw.PreferencesPage()
        page.set_title("Geral")
        page.set_icon_name("preferences-system-symbolic")
        self.add(page)

        # Grupo de diretório de download
        download_group = Adw.PreferencesGroup()
        download_group.set_title("Diretório de Download")
        page.add(download_group)

        # Pasta de download
        download_dir_row = Adw.ActionRow()
        download_dir_row.set_title("Pasta de download")
        download_dir_row.set_subtitle(self.settings.get("download_dir"))

        choose_button = Gtk.Button.new_with_label("Escolher")
        choose_button.connect("clicked", self.on_choose_dir_clicked, download_dir_row)
        download_dir_row.add_suffix(choose_button)
        download_group.add(download_dir_row)

        # Grupo de formatos
        format_group = Adw.PreferencesGroup()
        format_group.set_title("Formatos")
        page.add(format_group)

        # Formato padrão
        format_combo_row = Adw.ComboRow()
        format_combo_row.set_title("Formato padrão de vídeo")

        formats_model = Gtk.StringList()
        formats = ["mp4", "mkv", "webm"]
        for fmt in formats:
            formats_model.append(fmt)
        format_combo_row.set_model(formats_model)

        # Define o formato padrão
        default_format = self.settings.get('default_format', 'mp4')
        if default_format in formats:
            format_combo_row.set_selected(formats.index(default_format))

        format_group.add(format_combo_row)
        self.format_combo_row = format_combo_row

        # Grupo de notificações
        notify_group = Adw.PreferencesGroup()
        notify_group.set_title("Notificações")
        page.add(notify_group)

        # Tocar som ao concluir
        notify_sound_switch = Adw.SwitchRow()
        notify_sound_switch.set_title("Tocar som ao concluir download")
        notify_sound_switch.set_active(self.settings.get('notify_sound', True))
        notify_group.add(notify_sound_switch)
        self.notify_sound_switch = notify_sound_switch

        self.setup_buttons(page)

    def setup_appearance_page(self):
        """Configura a página de aparência"""
        page = Adw.PreferencesPage()
        page.set_title("Aparência")
        page.set_icon_name("applications-graphics-symbolic")
        self.add(page)

        # Grupo de tema
        theme_group = Adw.PreferencesGroup()
        theme_group.set_title("Tema")
        page.add(theme_group)

        # Modo escuro
        dark_mode_switch = Adw.SwitchRow()
        dark_mode_switch.set_title("Modo escuro")
        dark_mode_switch.set_active(self.settings.get('dark_mode', True))
        theme_group.add(dark_mode_switch)
        self.dark_mode_switch = dark_mode_switch

        # Grupo de interface
        ui_group = Adw.PreferencesGroup()
        ui_group.set_title("Interface")
        page.add(ui_group)

        # Lembrar tamanho da janela
        remember_size_switch = Adw.SwitchRow()
        remember_size_switch.set_title("Lembrar tamanho da janela")
        remember_size_switch.set_active(self.settings.get('remember_window_size', True))
        ui_group.add(remember_size_switch)
        self.remember_size_switch = remember_size_switch

        # Mostrar miniaturas
        preview_thumb_switch = Adw.SwitchRow()
        preview_thumb_switch.set_title("Mostrar miniaturas dos vídeos")
        preview_thumb_switch.set_active(self.settings.get('show_thumbnails', True))
        ui_group.add(preview_thumb_switch)
        self.preview_thumb_switch = preview_thumb_switch

        # Tamanho de fonte
        font_size_group = Adw.PreferencesGroup()
        font_size_group.set_title("Tamanho da fonte")
        page.add(font_size_group)

        font_size_row = Adw.ComboRow()
        font_size_row.set_title("Tamanho da fonte da interface")

        font_sizes_model = Gtk.StringList()
        font_sizes = ["10", "12", "14", "18", "22"]
        for size in font_sizes:
            font_sizes_model.append(size)
        font_size_row.set_model(font_sizes_model)

        current_font_size = self.settings.get('font_size', '12')
        if current_font_size in font_sizes:
            font_size_row.set_selected(font_sizes.index(current_font_size))

        font_size_group.add(font_size_row)
        self.font_size_row = font_size_row

        self.setup_buttons(page)

    def setup_advanced_page(self):
        """Configura a página de configurações avançadas"""
        page = Adw.PreferencesPage()
        page.set_title("Avançado")
        page.set_icon_name("applications-utilities-symbolic")
        self.add(page)

        # Grupo de desempenho
        performance_group = Adw.PreferencesGroup()
        performance_group.set_title("Desempenho")
        page.add(performance_group)

        adjustment = Gtk.Adjustment.new(
            value=3, lower=1, upper=10, step_increment=1,
            page_increment=1, page_size=0
        )

        # Downloads simultâneos
        concurrent_downloads_row = Adw.SpinRow(
            title="Número máximo de downloads",
            adjustment=adjustment
        )
        concurrent_downloads_row.set_value(self.settings.get('max_downloads', 3))
        concurrent_downloads_row.set_digits(0)
        performance_group.add(concurrent_downloads_row)
        self.concurrent_downloads_row = concurrent_downloads_row

        # Cache
        cache_group = Adw.PreferencesGroup()
        cache_group.set_title("Cache")
        page.add(cache_group)

        # Tamanho máximo do cache
        cache_size_row = Adw.ActionRow()
        cache_size_row.set_title("Tamanho máximo do cache")

        cache_size_entry = Gtk.SpinButton.new_with_range(100, 10000, 100)
        cache_size_entry.set_value(self.settings.get('max_cache_size_mb', 1000))
        cache_size_entry.set_valign(Gtk.Align.CENTER)

        size_label = Gtk.Label.new("MB")
        size_label.set_valign(Gtk.Align.CENTER)

        cache_size_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        cache_size_box.append(cache_size_entry)
        cache_size_box.append(size_label)

        cache_size_row.add_suffix(cache_size_box)
        cache_group.add(cache_size_row)
        self.cache_size_entry = cache_size_entry

        # Limpar cache
        clear_cache_button = Gtk.Button.new_with_label("Limpar Cache")
        clear_cache_button.connect("clicked", self.on_clear_cache_clicked)

        clear_cache_row = Adw.ActionRow()
        clear_cache_row.set_title("Limpar cache de miniaturas e metadados")
        clear_cache_row.add_suffix(clear_cache_button)
        cache_group.add(clear_cache_row)

        # Grupo de rede
        network_group = Adw.PreferencesGroup()
        network_group.set_title("Rede")
        page.add(network_group)

        # Usar proxy
        proxy_switch = Adw.SwitchRow()
        proxy_switch.set_title("Usar proxy")
        proxy_switch.set_active(self.settings.get('use_proxy', False))
        proxy_switch.connect("notify::active", self.on_proxy_switch_toggled)
        network_group.add(proxy_switch)
        self.proxy_switch = proxy_switch

        # Configurações de proxy
        self.proxy_expander_row = Adw.ExpanderRow()
        self.proxy_expander_row.set_title("Configurações de proxy")
        self.proxy_expander_row.set_sensitive(self.settings.get('use_proxy', False))
        network_group.add(self.proxy_expander_row)

        # Endereço do proxy
        proxy_host_row = Adw.EntryRow()
        proxy_host_row.set_title("Endereço")
        proxy_host_row.set_text(self.settings.get('proxy_host', ''))
        self.proxy_expander_row.add_row(proxy_host_row)
        self.proxy_host_row = proxy_host_row

        # Porta do proxy
        proxy_port_row = Adw.EntryRow()
        proxy_port_row.set_title("Porta")
        proxy_port_row.set_text(str(self.settings.get('proxy_port', '8080')))
        self.proxy_expander_row.add_row(proxy_port_row)
        self.proxy_port_row = proxy_port_row

        self.setup_buttons(page)

    def setup_buttons(self, page):
        # --- Botões Aplicar e Cancelar ---
        button_group = Adw.PreferencesGroup()
        page.add(button_group)

        button_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        button_box.set_halign(Gtk.Align.END)

        cancel_button = Gtk.Button.new_with_label("Cancelar")
        cancel_button.add_css_class("flat")
        cancel_button.connect("clicked", lambda btn: self.close())

        apply_button = Gtk.Button.new_with_label("Aplicar")
        apply_button.add_css_class("suggested-action")
        apply_button.connect("clicked", self.on_apply_clicked)

        button_box.append(cancel_button)
        button_box.append(apply_button)

        row = Adw.ActionRow()
        row.set_title("")  # Oculta título
        row.add_suffix(button_box)
        row.set_activatable(False)
        button_group.add(row)

    def on_proxy_switch_toggled(self, switch, param):
        """Habilita/desabilita as configurações de proxy"""
        self.proxy_expander_row.set_sensitive(switch.get_active())

    def on_choose_dir_clicked(self, button, row):
        """Manipula o clique no botão de escolher diretório"""
        dialog = Gtk.FileDialog.new()
        dialog.set_title("Escolha o diretório de download")
        dialog.set_initial_folder(Gio.File.new_for_path(self.settings.get('download_dir')))

        dialog.select_folder(None, None, self.on_folder_selected, row)

    def on_folder_selected(self, dialog, result, row):
        """Processa a seleção de diretório"""
        try:
            file = dialog.select_folder_finish(result)
            if file:
                path = file.get_path()
                row.set_subtitle(path)
                # Guarda temporariamente
                self.selected_download_dir = path
        except GLib.Error as error:
            print(f"Erro ao selecionar pasta: {error.message}")

    def on_clear_cache_clicked(self, button):
        """Limpa os arquivos de cache"""
        # Aqui seria implementada a lógica para limpar os caches
        dialog = Adw.AlertDialog()
        dialog.set_heading("Cache limpo")
        dialog.set_body("O cache de miniaturas e metadados foi limpo com sucesso.")
        dialog.add_response("ok", "OK")
        dialog.present()

    def on_apply_clicked(self, button):
        """Salva as configurações quando o botão Aplicar é clicado"""
        # Configurações gerais
        self.settings['default_format'] = ["mp4", "mkv", "webm"][self.format_combo_row.get_selected()]
        self.settings['notify_sound'] = self.notify_sound_switch.get_active()

        # Diretório de download (se tiver sido alterado)
        if hasattr(self, 'selected_download_dir'):
            self.settings['download_dir'] = self.selected_download_dir

        # Aparência
        self.settings['dark_mode'] = self.dark_mode_switch.get_active()
        self.settings['remember_window_size'] = self.remember_size_switch.get_active()
        self.settings['show_thumbnails'] = self.preview_thumb_switch.get_active()
        self.settings['font_size'] = ["10", "12", "14", "18", "22"][self.font_size_row.get_selected()]

        # Configurações avançadas
        self.settings['max_downloads'] = int(self.concurrent_downloads_row.get_value())
        self.settings['max_cache_size_mb'] = int(self.cache_size_entry.get_value())
        self.settings['use_proxy'] = self.proxy_switch.get_active()
        self.settings['proxy_host'] = self.proxy_host_row.get_text()
        self.settings['proxy_port'] = self.proxy_port_row.get_text()

        # Salva as configurações (implementação seria específica ao app)
        self.save_settings()
        self.close()

    def save_settings(self):
        """Método para salvar as configurações no arquivo de configuração"""
        # Esta implementação seria específica à sua aplicação

        if self.settings.save():
            # Aplica as novas configurações
            self.win.apply_theme()
        else:
            self.win.show_error_message("Erro ao salvar configurações.")
