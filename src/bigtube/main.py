#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Ponto de entrada principal para o aplicativo BigTube
"""
import sys
import gi
from bigtube.app import BigTubeApp

# Verifica dependências
try:
    import yt_dlp
except ImportError:
    print("Erro: yt-dlp não está instalado. Por favor, instale usando:")
    print("pip install yt-dlp")
    sys.exit(1)

# Verifica versões requeridas do GTK
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
gi.require_version('Gst', '1.0')
from gi.repository import Gst

# Inicializa GStreamer
Gst.init(None)


def main():
    """Função principal para iniciar o aplicativo"""
    app = BigTubeApp()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
