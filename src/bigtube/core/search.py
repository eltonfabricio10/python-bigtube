# -*- coding: utf-8 -*-
import yt_dlp
import json


class SearchEngine:
    """
    Classe responsável por se conectar ao yt-dlp e realizar pesquisas.
    """
    def __init__(self):
        # Configurações do yt-dlp para APENAS extrair informação (sem baixar)
        self.ydl_opts = {
            'format': 'bestaudio/best',
            'quiet': True,
            'extract_flat': 'in_playlist',
            'dump_single_json': True,
            'no_warnings': True,
        }

    def search_youtube(self, query: str, max_results: int = 10):
        """
        Busca no YouTube usando 'ytsearch' e retorna uma lista de resultados.
        """
        print(f"[SearchEngine] Executando yt-dlp para: {query}")
        search_query = f"ytsearch{max_results}:{query}"

        results = []
        try:
            # Usamos o yt-dlp como um módulo Python
            with yt_dlp.YoutubeDL(self.ydl_opts) as ydl:
                # Extrai as informações da query de busca
                info_dict = ydl.extract_info(search_query, download=False)

                # O 'info_dict' conterá uma chave 'entries' com os resultados
                if 'entries' in info_dict:
                    for entry in info_dict['entries']:
                        results.append({
                            'title': entry.get('title'),
                            'url': entry.get('webpage_url') or f"https://www.youtube.com/watch?v={entry.get('id')}",
                            'thumbnail': entry.get('thumbnail'),
                            'duration': entry.get('duration_string'),
                            'uploader': entry.get('uploader'),
                        })

        except Exception as e:
            print(f"[SearchEngine] Erro ao buscar: {e}")
            return []

        return results
