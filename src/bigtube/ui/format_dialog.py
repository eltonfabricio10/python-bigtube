import gi
from typing import Dict, Callable

gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Pango

# Internal Imports
from ..core.locales import ResourceManager as Res, StringKey


class FormatSelectionDialog(Adw.Window):
    """
    Modal dialog allowing the user to select video/audio quality.
    Uses Libadwaita PreferencesPage for a native look.
    """

    def __init__(self, parent_window, video_info: Dict, on_download_confirmed: Callable):
        super().__init__()

        # Window Configuration
        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_title(Res.get(StringKey.DIALOG_FORMAT_TITLE))
        self.set_default_size(500, 650)

        # Data & Callbacks
        self.callback = on_download_confirmed
        self.video_info = video_info

        # --- Layout Structure (Adw.ToolbarView) ---
        content_view = Adw.ToolbarView()
        self.set_content(content_view)

        # Top Bar
        header = Adw.HeaderBar()
        content_view.add_top_bar(header)

        # Scrollable Content (PreferencesPage)
        self.page = Adw.PreferencesPage()
        content_view.set_content(self.page)

        # --- Build UI Sections ---
        self._setup_header_info()

        # Video Section
        self._setup_section(
            title=Res.get(StringKey.LBL_VIDEO_FORMATS),
            items=video_info.get('videos', [])
        )

        # Audio Section
        self._setup_section(
            title=Res.get(StringKey.LBL_AUDIO_FORMATS),
            items=video_info.get('audios', [])
        )

    def _setup_header_info(self):
        """Creates the top section with Title and Duration."""
        group = Adw.PreferencesGroup()
        self.page.add(group)

        # Title Label (Large & Wrapped)
        lbl_title = Gtk.Label(label=self.video_info.get('title', Res.get(StringKey.LBL_UNTITLED)))
        lbl_title.set_wrap(True)
        lbl_title.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        lbl_title.set_justify(Gtk.Justification.CENTER)
        lbl_title.add_css_class("title-3")
        lbl_title.set_margin_bottom(4)

        # Duration Label
        raw_duration = self.video_info.get('duration')
        dur_txt = self._format_duration(raw_duration)
        dur = Res.get(StringKey.LBL_VIDEO_DURATION)
        lbl_dur = Gtk.Label(label=f"{dur} {dur_txt}")
        lbl_dur.add_css_class("dim-label")
        lbl_dur.set_margin_bottom(12)

        # Container
        box_header = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box_header.append(lbl_title)
        box_header.append(lbl_dur)
        group.set_header_suffix(box_header)

    def _setup_section(self, title: str, items: list):
        """Generates a list of ActionRows for formats."""
        count = len(items)
        group = Adw.PreferencesGroup()
        group.set_title(title)
        options_label = Res.get(StringKey.LBL_OPTIONS_AVAILABLE)
        group.set_description(f"{count} {options_label}")
        self.page.add(group)

        if count > 0:
            for item in items:
                row = self._create_action_row(item)
                group.add(row)
        else:
            # Empty State
            row_empty = Adw.ActionRow(title=Res.get(StringKey.LBL_NO_FORMATS_FOUND))
            group.add(row_empty)

    def _create_action_row(self, fmt_data: dict) -> Adw.ActionRow:
        """Creates a single selectable row."""
        row = Adw.ActionRow()
        row.set_title(fmt_data['label'])

        # Subtitle: Size • Codec
        codec = fmt_data.get('codec', 'N/A')
        size = fmt_data.get('size', '? MB')
        row.set_subtitle(f"{size} • {codec}")

        # Buttons Box
        box_btns = Gtk.Box(spacing=6)
        box_btns.set_valign(Gtk.Align.CENTER)

        # Schedule Button
        btn_schedule = Gtk.Button()
        btn_schedule.set_icon_name("alarm-symbolic")
        btn_schedule.add_css_class("flat")
        btn_schedule.set_tooltip_text(Res.get(StringKey.TIP_SCHEDULE_DOWNLOAD))
        btn_schedule.connect("clicked", lambda b, v=fmt_data: self._on_schedule_clicked(v))
        box_btns.append(btn_schedule)

        # Download Button
        btn = Gtk.Button(label=Res.get(StringKey.BTN_START_DOWNLOAD))
        btn.add_css_class("pill")
        btn.add_css_class("suggested-action")

        # Connect Signal
        # Using default arg v=fmt_data to capture the specific variable in the loop
        btn.connect("clicked", lambda b, v=fmt_data: self._on_item_clicked(v))

        box_btns.append(btn)

        row.add_suffix(box_btns)
        return row

    def _on_item_clicked(self, fmt_data):
        """Triggered when user clicks Download."""
        self.close()
        if self.callback:
            self.callback(self.video_info, fmt_data)

    def _on_schedule_clicked(self, fmt_data):
        """Triggered when user clicks Schedule."""
        # Open Schedule Dialog
        from .schedule_dialog import ScheduleDialog

        def on_time_selected(timestamp):
            # Pass timestamp to callback
            if self.callback:
                self.callback(self.video_info, fmt_data, timestamp)

        dlg = ScheduleDialog(self, on_time_selected)
        dlg.present()

    def _format_duration(self, seconds):
        """Formats seconds into H:MM:SS."""
        if not seconds:
            return "--:--"
        try:
            seconds = int(seconds)
        except (ValueError, TypeError):
            return "--:--"

        m, s = divmod(seconds, 60)
        h, m = divmod(m, 60)

        h_unit = Res.get(StringKey.LBL_HOURS_SHORT)
        m_unit = Res.get(StringKey.LBL_MINUTES_SHORT)
        s_unit = Res.get(StringKey.LBL_SECONDS_SHORT)

        if h > 0:
            return f"{h}{h_unit} {m}{m_unit} {s}{s_unit}"
        return f"{m}{m_unit} {s}{s_unit}"
