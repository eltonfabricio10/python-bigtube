import json
import subprocess
import os
import time
from .config import Config
from .updater import Updater


class DRMError(Exception):
    """Exceção personalizada para avisar a UI sobre erros de reprodução."""
    pass


class SearchEngine:
    def __init__(self):
        self.SEARCH_LIMIT = 10

        # 1. Garante que o binário (yt-dlp) e o runtime (Deno) existem
        Updater.ensure_exists()

        # Caminho do executável local
        self.binary = str(Config.YT_DLP_PATH)

    def search(self, query, source="youtube"):
        """
        Roteador simplificado de buscas.
        """
        query = query.strip()
        if not query:
            return []

        # ==============================================================================
        # 1. LINK DIRETO (Simples, sem estratégias de retry)
        # ==============================================================================
        if source == "url" or query.startswith("http") or query.startswith("www"):
            return self._handle_direct_link(query)

        # ==============================================================================
        # 2. BUSCA TEXTUAL
        # ==============================================================================

        force_audio = False
        args = []

        if source == "soundcloud":
            # --- MODO SOUNDCLOUD ---
            force_audio = True  # Marca como áudio (não abre janela)
            args = [
                "--flat-playlist",
                "--dump-json",
                f"scsearch{self.SEARCH_LIMIT}:{query}"
            ]

        else:
            # --- MODO YOUTUBE (Padrão para todo o resto) ---
            # Usa cliente Android que é rápido e estável para buscas
            args = [
                "--extractor-args", "youtube:player_client=android",
                "--flat-playlist",
                "--dump-json",
                f"ytsearch{self.SEARCH_LIMIT}:{query}"
            ]

        return self._run_cli(args, is_search=True, force_audio=force_audio)

    def _handle_direct_link(self, url):
        """
        Processa Links Diretos de forma simples.
        Usa o cliente 'android' que atualmente é o mais permissivo.
        """
        print(f"[Link] Processando: {url}")

        # Configuração única e robusta
        cmd_args = [
            url,
            "--dump-json",       # Apenas metadados JSON
            "--no-playlist",     # Tenta pegar apenas o vídeo se for lista
            "--skip-download",   # Não baixa o arquivo
            # Cliente Android evita erros de PO Token do iOS e SABR do Web
            "--extractor-args", "youtube:player_client=android",
            "--user-agent", "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Mobile Safari/537.36"
        ]

        try:
            # Executa uma única vez
            results = self._run_cli(cmd_args, is_search=False, force_audio=False)
            if results:
                return results
            else:
                raise DRMError("Nenhum dado retornado.")

        except Exception as e:
            # Se falhar, converte o erro para algo legível e avisa a UI
            self._raise_friendly_error(str(e))

    def _run_cli(self, args, is_search=True, force_audio=False):
        """
        Executa o yt-dlp via subprocesso, injetando o Deno no PATH.
        """
        full_cmd = [self.binary, "--ignore-errors", "--no-warnings"] + args

        # --- INJEÇÃO DO DENO ---
        # Adiciona a pasta bin (onde está o deno) no PATH temporário
        env = os.environ.copy()
        env["PATH"] = str(Config.BIN_DIR) + os.pathsep + env.get("PATH", "")

        try:
            process = subprocess.run(
                full_cmd,
                capture_output=True,
                text=True,
                encoding='utf-8',
                errors='replace',
                env=env  # <--- Ambiente com Deno
            )

            if process.returncode != 0 and not is_search:
                raise Exception(process.stderr)

            json_outputs = []
            for line in process.stdout.splitlines():
                line = line.strip()
                if not line: continue
                try:
                    data = json.loads(line)
                    json_outputs.append(self._parse_entry(data, force_audio))
                except json.JSONDecodeError:
                    pass

            return json_outputs

        except FileNotFoundError:
            raise DRMError("O executável do yt-dlp não foi encontrado.")

    def _raise_friendly_error(self, error_text):
        """Traduz erros técnicos."""
        err = error_text.lower()
        user_msg = "Não foi possível carregar."

        if "drm" in err: user_msg = "Conteúdo protegido por DRM."
        elif "geo" in err: user_msg = "Conteúdo bloqueado no país."
        elif "private" in err: user_msg = "Vídeo privado."
        elif "sign in" in err or "age" in err: user_msg = "Login necessário (Age Gated)."
        elif "403" in err: user_msg = "Acesso negado (403)."
        elif "404" in err: user_msg = "Não encontrado (404)."

        raise DRMError(f"{user_msg}\n(Log: {error_text[:100]}...)")

    def _parse_entry(self, entry, force_audio=False):
        """
        Normaliza os dados.
        """
        # 1. Resolve Thumbnail
        thumb_url = entry.get('thumbnail')
        if not thumb_url and 'thumbnails' in entry:
            thumbs = entry['thumbnails']
            if isinstance(thumbs, list) and len(thumbs) > 0:
                thumb_url = thumbs[-1].get('url')

        # 2. Lógica de Áudio vs Vídeo
        # Se force_audio=True (SoundCloud), is_video vira False.
        # Se force_audio=False, assume True, salvo se vcodec indicar 'none'.
        is_video = not force_audio

        if entry.get('vcodec') == 'none':
            is_video = False

        return {
            'title': entry.get('title', 'Sem Título'),
            'url': entry.get('webpage_url', entry.get('url', '')),
            'thumbnail': thumb_url,
            'uploader': entry.get('uploader', 'Desconhecido'),
            'duration': entry.get('duration', 0),
            'is_video': is_video
        }
