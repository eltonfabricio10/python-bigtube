export LD_LIBRARY_PATH="$APPDIR/usr/lib:$APPDIR/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export GST_PLUGIN_SCANNER="$APPDIR/usr/lib/gstreamer-1.0/gst-plugin-scanner"
export GST_PLUGIN_SCANNER_1_0="$APPDIR/usr/lib/gstreamer-1.0/gst-plugin-scanner"
export GST_PLUGIN_SYSTEM_PATH_1_0="$APPDIR/usr/lib/gstreamer-1.0"
export GST_PLUGIN_PATH_1_0="$APPDIR/usr/lib/gstreamer-1.0"
# BigTube is a libadwaita app. linuxdeploy's gtk hook forces GTK_THEME=Adwaita:<variant>,
# which layers the legacy GTK Adwaita CSS on top of libadwaita's own stylesheet and
# renders broken/ugly. Unset it so libadwaita styles natively (it still picks up
# light/dark from the desktop portal, which the gtk hook already queried).
unset GTK_THEME
