# -*- coding: utf-8 -*-
import gettext

# Config Gettext
LOCALE_DIR = "/usr/share/locale"
gettext.bindtextdomain("big-tube", LOCALE_DIR)
gettext.textdomain("big-tube")
_ = gettext.gettext
