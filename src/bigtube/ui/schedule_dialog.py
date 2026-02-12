
import gi
import time
from datetime import datetime
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GLib

from ..core.locales import ResourceManager as Res, StringKey

class ScheduleDialog(Adw.Window):
    """
    Modal dialog to select a future date and time for downloading.
    """
    def __init__(self, parent_window, on_confirm_callback):
        super().__init__()
        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_title(Res.get(StringKey.DLG_SCHEDULE_TITLE))
        self.set_default_size(400, 600)

        self.callback = on_confirm_callback

        # --- UI ---
        content = Adw.ToolbarView()
        self.set_content(content)

        # Header
        header = Adw.HeaderBar()
        content.add_top_bar(header)

        # Body
        page = Adw.PreferencesPage()
        content.set_content(page)

        group = Adw.PreferencesGroup()
        group.set_title(Res.get(StringKey.DLG_SCHEDULE_DATE_LABEL))
        page.add(group)

        # Calendar
        self.calendar = Gtk.Calendar()
        self.calendar.set_margin_bottom(10)
        # Wrap calendar in a box to center it or just add as row content
        cal_row = Adw.ActionRow()
        cal_row.set_title(Res.get(StringKey.DLG_SCHEDULE_DATE))
        cal_row.add_suffix(self.calendar)
        group.add(cal_row)

        # Time Spinners
        time_group = Adw.PreferencesGroup()
        time_group.set_title(Res.get(StringKey.DLG_SCHEDULE_TIME_LABEL))
        page.add(time_group)

        # Hours
        self.spin_hour = Gtk.SpinButton.new_with_range(0, 23, 1)
        self.spin_hour.set_valign(Gtk.Align.CENTER)

        # Minutes
        self.spin_min = Gtk.SpinButton.new_with_range(0, 59, 1)
        self.spin_min.set_valign(Gtk.Align.CENTER)

        row_time = Adw.ActionRow()
        row_time.set_title(Res.get(StringKey.DLG_SCHEDULE_TIME_FMT))

        box_time = Gtk.Box(spacing=6)
        box_time.append(self.spin_hour)
        box_time.append(Gtk.Label(label=":"))
        box_time.append(self.spin_min)

        row_time.add_suffix(box_time)
        time_group.add(row_time)

        # Set current time + 1 hour default
        now = datetime.now()
        self.spin_hour.set_value(now.hour)
        self.spin_min.set_value(now.minute)

        # Confirm Button
        btn_confirm = Gtk.Button(label=Res.get(StringKey.BTN_SCHEDULE))
        btn_confirm.add_css_class("pill")
        btn_confirm.add_css_class("suggested-action")
        btn_confirm.set_margin_top(20)
        btn_confirm.set_halign(Gtk.Align.CENTER)
        btn_confirm.connect("clicked", self._on_confirm)

        # Add button to key content/page
        # Since PreferencesPage expects groups, we can put button in a separate group or custom bin
        # Hack: Add a group with a custom child
        btn_group = Adw.PreferencesGroup()
        btn_group.add(btn_confirm)
        page.add(btn_group)

    def _on_confirm(self, btn):
        # Calculate Timestamp
        date = self.calendar.get_date() # GLib.DateTime
        year = date.get_year()
        month = date.get_month()
        day = date.get_day_of_month()

        hour = int(self.spin_hour.get_value())
        minute = int(self.spin_min.get_value())

        # Construct datetime
        # Gtk.Calendar months are 1-12
        dt = datetime(year, month, day, hour, minute)
        ts = dt.timestamp()

        if ts < time.time():
            # Warn if in past? For now assume user knows or it runs immediately
            pass

        self.close()
        if self.callback:
            self.callback(ts)
