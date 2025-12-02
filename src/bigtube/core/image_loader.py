import threading
from concurrent.futures import ThreadPoolExecutor
from urllib.request import urlopen, Request
from gi.repository import GdkPixbuf, GLib


class ImageLoader:
    # --- O SEGREDO DO SUCESSO ---
    # Cria um "Pool" fixo de 4 trabalhadores.
    # Se chegarem 50 imagens, elas entram na fila e não travam o PC.
    _executor = ThreadPoolExecutor(max_workers=16, thread_name_prefix="ImgLoader")

    @classmethod
    def load(cls, url, image_widget, width, height):
        """
        Agenda o download da imagem sem travar a interface.
        """
        # 1. Validação Rápida (Sincrona)
        # Se não for link válido, define ícone padrão e aborta.
        if not url or not isinstance(url, str) or not url.startswith('http'):
            cls._set_placeholder(image_widget, width)
            return

        # 2. Agenda o trabalho para o Pool (Assíncrono)
        # O submit retorna imediatamente, não trava a UI.
        cls._executor.submit(cls._download_task, url, image_widget, width, height)

    @staticmethod
    def _download_task(url, image_widget, width, height):
        try:
            # Configura timeout curto (5s) para não prender o app ao fechar
            req = Request(url, headers={'User-Agent': 'Mozilla/5.0'})
            response = urlopen(req, timeout=5)
            data = response.read()

            # Carrega a imagem na memória
            loader = GdkPixbuf.PixbufLoader()
            loader.write(data)
            loader.close()
            pixbuf = loader.get_pixbuf()

            if pixbuf:
                # Redimensiona
                pixbuf = pixbuf.scale_simple(width, height, GdkPixbuf.InterpType.BILINEAR)
                # Atualiza UI (Seguro)
                GLib.idle_add(image_widget.set_from_pixbuf, pixbuf)
            else:
                raise ValueError("Pixbuf vazio")

        except Exception:
            # Se der erro (timeout, 404), põe o placeholder
            GLib.idle_add(lambda: ImageLoader._set_placeholder(image_widget, width))

    @staticmethod
    def _set_placeholder(image_widget, size):
        # Fallback seguro
        try:
            image_widget.set_from_icon_name('image-missing-symbolic')
            image_widget.set_pixel_size(size)
        except Exception:
            pass

    @classmethod
    def shutdown(cls):
        """Mata o pool quando o app fecha."""
        cls._executor.shutdown(wait=False, cancel_futures=True)
