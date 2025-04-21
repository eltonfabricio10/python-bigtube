# -*- coding: utf-8 -*-
"""
Funções e classes utilitárias para o BigTube
"""
import os
import sys
import re
import subprocess
import tempfile
import threading
import shutil
import uuid
import urllib.parse
from pathlib import Path
from datetime import datetime

import gi
gi.require_version('Gst', '1.0')
from gi.repository import Gst, GLib


class DownloadLogger:
    """Logger personalizado para capturar o progresso do download"""

    def __init__(self):
        self.callback = None
        self.error_callback = None

    def debug(self, msg):
        """Captura mensagens de debug, especialmente progresso de download"""
        if msg.startswith('[download]'):
            match = re.search(r'\b(\d+\.\d+)%', msg)
            if match:
                percentage = float(match.group(1))
                if hasattr(self, 'callback') and self.callback:
                    self.callback(percentage)

    def info(self, msg):
        """Captura mensagens informativas"""
        pass

    def warning(self, msg):
        """Captura mensagens de aviso"""
        if hasattr(self, 'warning_callback') and self.warning_callback:
            self.warning_callback(msg)

    def error(self, msg):
        """Captura mensagens de erro"""
        if hasattr(self, 'error_callback') and self.error_callback:
            self.error_callback(msg)


def validate_url(url):
    """Valida se a string é uma URL válida"""
    try:
        result = urllib.parse.urlparse(url)
        return all([
            result.scheme in ['http', 'https', 'ftp'],
            result.netloc
        ])
    except Exception:
        return False


def sanitize_filename(filename):
    """Sanitiza um nome de arquivo removendo caracteres inválidos"""
    # Remove caracteres não permitidos em sistemas de arquivos
    invalid_chars = r'[<>:"/\\|?*\x00-\x1F]'
    sanitized = re.sub(invalid_chars, '_', filename)

    # Limita o tamanho do nome de arquivo
    if len(sanitized) > 200:
        base, ext = os.path.splitext(sanitized)
        sanitized = base[:196] + ext

    return sanitized


def generate_unique_filename(directory, basename, extension):
    """Gera um nome de arquivo único se já existir um com o mesmo nome"""
    original = os.path.join(directory, f"{basename}.{extension}")

    if not os.path.exists(original):
        return original

    # Se já existe, adiciona um timestamp
    timestamp = datetime.now().strftime("%Y%m%d%H%M%S")
    return os.path.join(directory, f"{basename}_{timestamp}.{extension}")


def check_disk_space(directory, required_mb=500):
    """Verifica se há espaço suficiente em disco"""
    try:
        stats = os.statvfs(directory)
        free_bytes = stats.f_frsize * stats.f_bavail
        free_mb = free_bytes / (1024 * 1024)
        return free_mb >= required_mb
    except Exception:
        # Em caso de erro, assumimos que há espaço suficiente
        return True


def play_notification_sound():
    """Toca um som de notificação"""
    try:
        # Usa GStreamer para tocar um som
        player = Gst.ElementFactory.make("playbin", "player")

        sound_paths = [
            "/usr/share/sounds/freedesktop/stereo/complete.oga",
            "/usr/share/sounds/ubuntu/stereo/dialog-information.ogg",
            "/usr/share/sounds/gnome/default/alerts/complete.ogg"
        ]

        for sound_path in sound_paths:
            if os.path.exists(sound_path):
                player.set_property("uri", f"file://{sound_path}")
                player.set_state(Gst.State.PLAYING)
                break
    except Exception as e:
        print(f"Erro ao tocar som: {e}")


def open_file(path):
    """Abre um arquivo com o aplicativo padrão"""
    try:
        subprocess.Popen(['xdg-open', path])
        return True
    except Exception as e:
        print(f"Erro ao abrir arquivo: {e}")
        return False


def fetch_video_thumbnail(url, callback):
    """Busca a miniatura de um vídeo"""
    def _fetch_thumbnail():
        try:
            import yt_dlp
            with yt_dlp.YoutubeDL({'quiet': True}) as ydl:
                info = ydl.extract_info(url, download=False)
                thumbnails = info.get('thumbnails', [])

                # Procura por uma miniatura de tamanho adequado
                thumb_url = None
                for thumb in thumbnails:
                    # Prefere miniaturas médias
                    if thumb.get('height', 0) >= 120 and thumb.get('height', 0) <= 480:
                        thumb_url = thumb.get('url')
                        break

                # Se não encontrou uma no tamanho adequado, usa a primeira
                if not thumb_url and thumbnails:
                    thumb_url = thumbnails[0].get('url')

                # Baixa a miniatura para um arquivo temporário
                if thumb_url:
                    temp_file = tempfile.NamedTemporaryFile(delete=False, suffix='.png')
                    temp_file.close()

                    import urllib.request
                    urllib.request.urlretrieve(thumb_url, temp_file.name)

                    # Chama o callback na thread principal
                    GLib.idle_add(lambda: callback(temp_file.name) and False)
                    return

            # Se chegou aqui, não conseguiu obter a miniatura
            GLib.idle_add(lambda: callback(None) and False)

        except Exception as e:
            print(f"Erro ao buscar miniatura: {e}")
            GLib.idle_add(lambda: callback(None) and False)

    # Executa em uma thread separada
    thread = threading.Thread(target=_fetch_thumbnail)
    thread.daemon = True
    thread.start()

    return thread
