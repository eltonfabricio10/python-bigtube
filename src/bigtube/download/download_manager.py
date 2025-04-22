# -*- coding: utf-8 -*-
"""
Gerenciamento de downloads de vídeos
"""
import os
import threading
import subprocess
import json
from datetime import datetime
import gi
import yt_dlp
from bigtube.utils import (
    DownloadLogger,
    sanitize_filename,
    generate_unique_filename,
    check_disk_space,
    play_notification_sound
)

gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import GLib


class DownloadItem:
    """Representa um item de download com todos os seus dados"""

    def __init__(self, url, download_dir, options=None):
        self.url = url
        self.download_dir = download_dir
        self.options = options or {}
        self.id = f"download_{datetime.now().strftime('%Y%m%d%H%M%S')}"
        self.title = "Download pendente"
        self.output_file = None
        self.cancelled = False
        self.completed = False
        self.error = None
        self.progress = 0.0
        self.status = "Pendente"
        self.start_time = None
        self.end_time = None
        self.ydl = None  # Referência ao objeto YoutubeDL

    def to_dict(self):
        """Converte o item para um dicionário"""
        return {
            'id': self.id,
            'url': self.url,
            'title': self.title,
            'output_file': self.output_file,
            'completed': self.completed,
            'error': self.error,
            'progress': self.progress,
            'status': self.status,
            'start_time': self.start_time.isoformat() if self.start_time else None,
            'end_time': self.end_time.isoformat() if self.end_time else None
        }

    @classmethod
    def from_dict(cls, data, download_dir):
        """Cria um item a partir de um dicionário"""
        item = cls(data['url'], download_dir)
        item.id = data['id']
        item.title = data['title']
        item.output_file = data['output_file']
        item.completed = data['completed']
        item.error = data['error']
        item.progress = data['progress']
        item.status = data['status']

        if data['start_time']:
            item.start_time = datetime.fromisoformat(data['start_time'])
        if data['end_time']:
            item.end_time = datetime.fromisoformat(data['end_time'])

        return item


class DownloadManager:
    """Gerencia os downloads de vídeos"""

    def __init__(self, config):
        self.config = config
        self.active_downloads = []  # Downloads ativos
        self.download_history = []  # Histórico de downloads
        self.download_threads = []  # Threads de download
        self.callbacks = {
            'progress_update': [],
            'download_complete': [],
            'download_error': [],
            'download_start': []
        }

        # Carrega o histórico de downloads
        self.semaphore = threading.Semaphore(config.get('max_downloads', 3))
        self.load_history()

    def add_callback(self, event_type, callback):
        """Adiciona um callback para um tipo de evento"""
        if event_type in self.callbacks:
            self.callbacks[event_type].append(callback)

    def remove_callback(self, event_type, callback):
        """Remove um callback"""
        if event_type in self.callbacks and callback in self.callbacks[event_type]:
            self.callbacks[event_type].remove(callback)

    def _trigger_callbacks(self, event_type, *args):
        """Dispara todos os callbacks para um tipo de evento"""
        for callback in self.callbacks.get(event_type, []):
            callback(*args)

    def load_history(self):
        """Carrega o histórico de downloads"""
        history_file = os.path.join(
            os.path.expanduser("~"),
            ".config",
            "bigtube",
            "download_history.json"
        )

        if os.path.exists(history_file):
            try:
                with open(history_file, 'r') as f:
                    history_data = json.load(f)

                for item_data in history_data:
                    item = DownloadItem.from_dict(item_data, self.config['download_dir'])
                    self.download_history.append(item)
            except Exception as e:
                print(f"Erro ao carregar histórico: {e}")

    def save_history(self):
        """Salva o histórico de downloads"""
        history_file = os.path.join(
            os.path.expanduser("~"),
            ".config",
            "bigtube",
            "download_history.json"
        )

        # Garante que o diretório existe
        os.makedirs(os.path.dirname(history_file), exist_ok=True)

        try:
            history_data = [item.to_dict() for item in self.download_history]
            with open(history_file, 'w') as f:
                json.dump(history_data, f, indent=2)
        except Exception as e:
            print(f"Erro ao salvar histórico: {e}")

    def start_download(self, url, download_row, audio_only=False, file_format="mp4", quality="Melhor"):
        """Inicia um download de vídeo"""
        with self.semaphore:
            # Cria um novo item de download
            download_item = DownloadItem(url, self.config['download_dir'], {
                'audio_only': audio_only,
                'format': file_format,
                'quality': quality
            })

            # Adiciona aos downloads ativos
            self.active_downloads.append(download_item)

            # Verifica espaço em disco
            if not check_disk_space(self.config['download_dir']):
                error_msg = "Espaço em disco insuficiente para o download."
                GLib.idle_add(self._download_error, download_item, download_row, error_msg)
                return

            # Iniciar o download em uma thread separada
            thread = threading.Thread(
                target=self._download_video_thread,
                args=(download_item, download_row)
            )
            thread.daemon = True
            self.download_threads.append(thread)
            thread.start()

            # Notifica os callbacks
            self._trigger_callbacks('download_start', download_item)

            return download_item

    def cancel_download(self, download_item, download_row):
        """Cancela um download em andamento"""
        if download_item in self.active_downloads and not download_item.completed:
            download_item.cancelled = True
            GLib.idle_add(
                self.update_download_progress,
                download_item, download_row, 0.0,
                "Cancelando download..."
            )

            # Tenta interromper o yt-dlp
            if download_item.ydl:
                try:
                    # yt-dlp não tem um método direto para interromper,
                    # mas podemos tentar forçar uma exceção
                    download_item.ydl._finish_multiline_status()
                    download_item.status = "Download cancelado!"
                except:
                    pass

    def clean_finished_threads(self):
        """Remove threads que já terminaram"""
        self.download_threads = [t for t in self.download_threads if t.is_alive()]
        if len(self.download_history) > 30:
            self.download_history = self.download_history[-30:]

    def update_download_progress(self, download_item, download_row, fraction, status_text):
        """Atualiza o progresso do download na UI"""
        download_item.progress = fraction
        download_item.status = status_text

        # Atualiza a UI
        download_row.progress_bar.set_fraction(fraction)
        download_row.status_label.set_text(status_text)

        # Notifica callbacks
        self._trigger_callbacks('progress_update', download_item)

        return False  # Remove da fila de idle

    def _download_finished(self, download_item, download_row, output_file):
        """Chamado quando um download é concluído"""
        download_item.completed = True
        download_item.output_file = output_file
        download_item.progress = 1.0
        download_item.status = f"Download concluído: {os.path.basename(output_file)}"
        download_item.end_time = datetime.now()

        # Atualiza a UI
        download_row.progress_bar.set_fraction(1.0)
        download_row.status_label.set_text(download_item.status)
        download_row.play_button.set_sensitive(True)
        download_row.open_folder_button.set_sensitive(True)
        download_row.cancel_button.set_sensitive(False)

        # Toca som de notificação
        if self.config['notify_sound']:
            play_notification_sound()

        # Adiciona ao histórico
        if download_item not in self.download_history:
            self.download_history.append(download_item)
            self.save_history()

        # Remove dos downloads ativos
        if download_item in self.active_downloads:
            self.active_downloads.remove(download_item)

        # Notifica callbacks
        self._trigger_callbacks('download_complete', download_item)

        # Limpa threads finalizadas
        self.clean_finished_threads()

        return False

    def _download_error(self, download_item, download_row, error_message):
        """Chamado quando ocorre um erro no download"""
        download_item.error = error_message
        download_item.status = f"Erro: {error_message}"
        download_item.end_time = datetime.now()

        # Atualiza a UI
        download_row.progress_bar.add_css_class("error")
        download_row.status_label.set_text(download_item.status)
        download_row.cancel_button.set_sensitive(False)

        # Adiciona ao histórico
        if download_item not in self.download_history:
            self.download_history.append(download_item)
            self.save_history()

        # Remove dos downloads ativos
        if download_item in self.active_downloads:
            self.active_downloads.remove(download_item)

        # Notifica callbacks
        self._trigger_callbacks('download_error', download_item, error_message)

        # Limpa threads finalizadas
        self.clean_finished_threads()

        return False

    def _download_cancelled(self, download_item, download_row):
        """Chamado quando um download é cancelado"""
        download_item.status = "Download cancelado"
        download_item.end_time = datetime.now()

        # Atualiza a UI
        download_row.progress_bar.set_fraction(0.0)
        download_row.status_label.set_text(download_item.status)
        download_row.cancel_button.set_sensitive(False)

        # Adiciona ao histórico
        if download_item not in self.download_history:
            self.download_history.append(download_item)
            self.save_history()

        # Remove dos downloads ativos
        if download_item in self.active_downloads:
            self.active_downloads.remove(download_item)

        # Limpa threads finalizadas
        self.clean_finished_threads()

        return False

    def _download_progress_hook(self, download_item, download_row):
        """Hook para atualizar o progresso do download"""
        def hook(d):
            if download_item.cancelled:
                GLib.idle_add(self._download_cancelled, download_item, download_row)
                return

            if d['status'] == 'downloading':
                # Extrair dados do progresso
                try:
                    downloaded = d.get('downloaded_bytes', 0)
                    total = d.get('total_bytes', 0)
                    if total == 0:
                        total = d.get('total_bytes_estimate', 0)

                    if total > 0:
                        percent = downloaded / total
                        speed = d.get('speed', 0)
                        eta = d.get('eta', 0)

                        if speed:
                            speed_str = f"{speed/1024/1024:.2f} MB/s"
                        else:
                            speed_str = "-- MB/s"

                        if eta:
                            eta_str = f"{eta} segundos"
                        else:
                            eta_str = "-- segundos"

                        status_text = f"Baixando... {percent*100:.1f}% ({speed_str}, ETA: {eta_str})"
                        GLib.idle_add(self.update_download_progress, download_item, download_row, percent, status_text)
                    else:
                        # Sem informação de tamanho total, mostrar bytes baixados
                        mb_downloaded = downloaded / 1024 / 1024
                        status_text = f"Baixando... {mb_downloaded:.2f} MB"
                        # Aqui usamos um indicador de progresso indeterminado
                        GLib.idle_add(self.update_download_progress, download_item, download_row, 0.0, status_text)
                        # Fazemos a barra pulsar
                        download_row.progress_bar.pulse()

                except Exception as e:
                    print(f"Erro ao processar progresso: {e}")

            elif d['status'] == 'finished':
                GLib.idle_add(self.update_download_progress, download_item, download_row, 1.0, "Processando vídeo...")

            elif d['status'] == 'error':
                error_msg = d.get('error', 'Erro desconhecido')
                GLib.idle_add(self._download_error, download_item, download_row, error_msg)

        return hook

    def _download_video_thread(self, download_item, download_row):
        """Download real do vídeo usando yt-dlp"""
        download_item.start_time = datetime.now()

        try:
            # Configuração do logger para capturar o progresso
            logger = DownloadLogger()

            # Função para atualizar o progresso na UI
            def progress_hook(percentage):
                GLib.idle_add(self.update_download_progress,
                              download_item, download_row, percentage/100.0,
                              f"Baixando... {percentage:.1f}%")

            def error_hook(error_msg):
                GLib.idle_add(self._download_error, download_item, download_row, error_msg)

            # Configura callbacks
            logger.callback = progress_hook
            logger.error_callback = error_hook

            # Diretório de download
            download_dir = self.config['download_dir']
            os.makedirs(download_dir, exist_ok=True)

            # Determina o formato baseado nas opções
            audio_only = download_item.options.get('audio_only', False)
            file_format = download_item.options.get('format', 'mp4')
            quality = download_item.options.get('quality', 'Melhor')

            if audio_only:
                # Para somente áudio, usamos M4A ou MP3
                format_str = 'bestaudio[ext=m4a]/bestaudio/best'
                postprocessors = [{
                    'key': 'FFmpegExtractAudio',
                    'preferredcodec': 'mp3',
                    'preferredquality': '192',
                }]
            else:
                # Para vídeo, escolhemos baseado na qualidade
                if quality == "Melhor":
                    format_str = f'bestvideo[ext={file_format}]+bestaudio/best[ext={file_format}]/best'
                else:
                    # Remove o 'p' de '1080p', etc.
                    res = quality.rstrip('p')
                    format_str = f'bestvideo[height<={res}][ext={file_format}]+bestaudio/best[height<={res}][ext={file_format}]/best'

                postprocessors = [{
                    'key': 'FFmpegVideoConvertor',
                    'preferedformat': file_format,
                }]

            # Opções para o yt-dlp
            ydl_opts = {
                'format': format_str,
                'outtmpl': os.path.join(download_dir, '%(title)s.%(ext)s'),
                'postprocessors': postprocessors,
                'logger': logger,
                'progress_hooks': [self._download_progress_hook(download_item, download_row)],
                'noplaylist': True,  # Não baixar playlists inteiras
                'quiet': True,       # Minimizar saída no terminal
            }

            # Armazena referência para possível cancelamento
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                download_item.ydl = ydl

                # Verificar se já foi cancelado
                if download_item.cancelled:
                    GLib.idle_add(self._download_cancelled, download_item, download_row)
                    return

                # Obter informações do vídeo
                GLib.idle_add(self.update_download_progress,
                              download_item, download_row, 0.0,
                              "Obtendo informações do vídeo...")

                info = ydl.extract_info(download_item.url, download=False)
                video_title = info.get('title', 'Vídeo')

                # Sanitiza o título para uso como nome de arquivo
                safe_title = sanitize_filename(video_title)
                download_item.title = video_title

                # Atualiza o título da linha de download
                GLib.idle_add(lambda: download_row.set_title(f"Download: {video_title}"))

                # Iniciar download
                if not download_item.cancelled:
                    GLib.idle_add(self.update_download_progress,
                                  download_item, download_row, 0.05,
                                  "Download iniciado...")

                    # Fazer o download real
                    ydl.download([download_item.url])

                    # Determinar o arquivo final
                    file_ext = 'mp3' if audio_only else file_format
                    expected_filename = f"{safe_title}.{file_ext}"

                    # Caminho completo do arquivo
                    output_file = os.path.join(download_dir, expected_filename)

                    # Verificar se o arquivo foi criado
                    if not os.path.exists(output_file):
                        # Procurar qualquer arquivo que comece com o título
                        for file in os.listdir(download_dir):
                            if file.startswith(safe_title[:20]):  # Usa parte do título para comparação
                                output_file = os.path.join(download_dir, file)
                                break

                    # Se ainda não encontrou, gerar um nome único
                    if not os.path.exists(output_file):
                        output_file = generate_unique_filename(download_dir, safe_title, file_ext)

                    download_item.output_file = output_file

                    if not download_item.cancelled:
                        GLib.idle_add(self._download_finished, download_item, download_row, output_file)
                    else:
                        GLib.idle_add(self._download_cancelled, download_item, download_row)

        except Exception as e:
            import traceback
            traceback.print_exc()
            GLib.idle_add(self._download_error, download_item, download_row, str(e))
