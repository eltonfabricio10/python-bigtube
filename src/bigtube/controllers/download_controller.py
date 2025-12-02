from ..ui.download_row import DownloadRow


class DownloadController:
    def __init__(self, list_box_widget, on_play_callback):
        """
        Recebe os widgets REAIS que estão na MainWindow.
        """
        self.list_box = list_box_widget
        self.on_play_callback = on_play_callback
        self.active_downloads = {}

    def add_download(self, title, filename, url, format_id, full_path):
        # Cria a linha (Row) - Que continua separada pois é repetitiva
        row = DownloadRow(title, filename, full_path, self.on_play_callback)

        # Adiciona na lista que veio da MainWindow
        self.list_box.append(row)
        self.active_downloads[url] = row
        return row

    def update_status(self, url, percent, status):
        if url in self.active_downloads:
            self.active_downloads[url].update_progress(percent, status)
