import os
import json
import subprocess
from .config import Config
from .updater import Updater


class Downloader:
    def __init__(self):
        Updater.ensure_exists()
        Config.load()
        self.binary = str(Config.YT_DLP_PATH)

    def fetch_video_info(self, url):
        """
        Consulta os metadados COMPLETOS.
        """
        print(f"[Downloader] Extraindo formatos de: {url}")

        cmd = [
            self.binary,
            "--dump-single-json",
            "--no-warnings",
            # MUDANÇA 1: Usamos 'tv_embedded' aqui porque ele expõe
            # as resoluções (1080p, 720p) separadamente de forma mais clara que o Android.
            "--extractor-args", "youtube:player_client=tv_embedded,web_embedded",
            url
        ]

        env = os.environ.copy()
        env["PATH"] = str(Config.BIN_DIR) + os.pathsep + env.get("PATH", "")

        try:
            process = subprocess.run(
                cmd, capture_output=True, text=True, encoding='utf-8',
                errors='replace', env=env
            )

            if process.returncode != 0:
                print(f"[Downloader Error Stderr] {process.stderr}")
                raise Exception("Erro ao ler formatos.")

            info = json.loads(process.stdout)
            return self._parse_formats(info)

        except Exception as e:
            print(f"[Downloader Exception] {e}")
            return None

    def _parse_formats(self, info):
        """
        Parser agressivo: Tenta aceitar tudo que parecer áudio ou vídeo.
        """
        duration = info.get('duration', 0)

        clean_data = {
            'id': info.get('id'),
            'title': info.get('title'),
            'url': info.get('webpage_url') or info.get('url'),
            'thumbnail': info.get('thumbnail'),
            'duration': duration,
            'videos': [],
            'audios': []
        }

        formats = info.get('formats', [])

        # DEBUG: Ver quantos formatos brutos chegaram
        print(f"[Downloader] Formatos brutos encontrados: {len(formats)}")

        for f in formats:
            # Filtros básicos de lixo
            if 'storyboard' in f.get('format_note', '') or f.get('protocol') == 'http_dash_segments':
                continue

            fmt_id = f.get('format_id')
            ext = f.get('ext')

            # Pega valores brutos (podem ser None)
            vcodec = f.get('vcodec')
            acodec = f.get('acodec')
            height = f.get('height')

            # --- CÁLCULO DE TAMANHO ---
            filesize = f.get('filesize') or f.get('filesize_approx')
            # Se não tem tamanho, calcula pelo Bitrate (tbr = total bitrate)
            if not filesize and f.get('tbr') and duration:
                filesize = (f.get('tbr') * 1024 / 8) * duration

            size_mb = (filesize / 1024 / 1024) if filesize else 0
            size_str = f"{size_mb:.1f} MB" if size_mb > 0 else "? MB"

            # --- LÓGICA DE CLASSIFICAÇÃO (CORRIGIDA) ---

            # É ÁUDIO SE: vcodec é 'none' OU vcodec é None (null no json)
            # E precisa ter acodec válido.
            is_audio_only = (vcodec == 'none' or vcodec is None) and (acodec != 'none' and acodec is not None)

            # É VÍDEO SE: tem altura definida (height > 0)
            is_video = height is not None and height > 0

            # --- 1. PROCESSAR ÁUDIO ---
            if is_audio_only:
                abr = f.get('abr') or 0
                clean_data['audios'].append({
                    'id': fmt_id,
                    'label': f"Áudio {ext.upper()} - {int(abr)}kbps",
                    'ext': ext,
                    'size': size_str,
                    'size_val': size_mb,
                    'type': 'audio',
                    'quality': abr
                })

            # --- 2. PROCESSAR VÍDEO ---
            elif is_video:
                fps = f.get('fps') or 0

                # Monta Label
                label_parts = [f"{height}p"]
                if fps > 30: label_parts.append(f"{int(fps)}fps")
                label_parts.append(f"({ext})")

                # Codec Info
                vc = str(vcodec).lower()
                if 'av01' in vc: label_parts.append("[AV1]")
                elif 'vp9' in vc: label_parts.append("[VP9]")
                elif 'avc1' in vc or 'h264' in vc: label_parts.append("[H.264]")

                if f.get('dynamic_range') == 'HDR': label_parts.append("HDR")

                clean_data['videos'].append({
                    'id': fmt_id,
                    'label': " ".join(label_parts),
                    'resolution': height,
                    'fps': fps,
                    'ext': ext,
                    'size': size_str,
                    'size_val': size_mb,
                    'type': 'video'
                })

        # --- ORDENAÇÃO E LIMPEZA ---

        # Remove duplicatas exatas de label para limpar a lista visual
        # (Ex: às vezes tem 2 streams H.264 iguais de fontes diferentes)
        clean_data['videos'] = self._remove_duplicates(clean_data['videos'])
        clean_data['audios'] = self._remove_duplicates(clean_data['audios'])

        # Ordena Vídeos: Resolução > FPS > Tamanho
        clean_data['videos'].sort(
            key=lambda x: (x['resolution'], x['fps'], x['size_val']),
            reverse=True
        )

        # Ordena Áudios: Qualidade > Tamanho
        clean_data['audios'].sort(
            key=lambda x: (x['quality'], x['size_val']),
            reverse=True
        )

        return clean_data

    def _remove_duplicates(self, items):
        seen = set()
        unique = []
        for item in items:
            # Chave única: Label + Extensão + Tamanho Aprox
            key = (item['label'], item['ext'], int(item['size_val']))
            if key not in seen:
                unique.append(item)
                seen.add(key)
        return unique

    def download_video(self, url, format_id, title, progress_callback=None):
        """
        Baixa o vídeo e reporta erros detalhados.
        """
        print(f"[Downloader] Iniciando download: {title} (ID: {format_id})")
        self.download_folder = Config.get("download_path")

        if not os.path.exists(self.download_folder):
            os.makedirs(self.download_folder)

        # 1. Sanitização de Nome
        safe_title = "".join([c for c in title if c.isalnum() or c in " -_()."]).strip()
        if not safe_title: safe_title = f"video_{format_id}"

        output_template = os.path.join(self.download_folder, f"{safe_title}.%(ext)s")

        # 2. Lógica de Áudio Inteligente (O Pulo do Gato 🐈)
        # Se o ID for numérico (vídeo), pedimos para colar o áudio junto.
        # Se for string (ex: '140' ou 'ba' - áudio), baixamos só ele.
        # O yt-dlp entende: "baixe ESSE video E o melhor áudio que achar"

        # Como saber se é vídeo ou áudio?
        # Simples: Se o usuário escolheu no popup, nós sabemos, mas aqui recebemos só o ID.
        # Estratégia Sênior: Tentamos baixar '{id}+bestaudio/best'.
        # Se for áudio puro, o yt-dlp ignora o '+bestaudio' ou baixa e mescla (sem problemas).
        # Mas para garantir, vamos assumir que queremos juntar áudio sempre que possível.

        final_format_arg = f"{format_id}+bestaudio/best"

        # Mas se o usuário escolheu ÁUDIO na lista (m4a, mp3), não queremos vídeo junto.
        # Vamos confiar que se o usuário escolheu um formato de vídeo, ele quer som.
        # Nota: Se o formato original já tiver som, o yt-dlp é esperto e não duplica.

        cmd = [
            self.binary,
            "--no-warnings",
            "--newline",
            "-f", final_format_arg, # <--- AQUI MUDOU (Garante som)
            "--merge-output-format", "mp4", # <--- Garante que o final seja MP4/MKV compatível
            "-o", output_template,

            # MUDANÇA CRÍTICA: Mesmos clientes da busca para os IDs baterem
            "--extractor-args", "youtube:player_client=tv_embedded,web_embedded",

            "--ignore-config",
            url
        ]

        env = os.environ.copy()
        env["PATH"] = str(Config.BIN_DIR) + os.pathsep + env.get("PATH", "")

        error_log = []

        try:
            process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding='utf-8',
                errors='replace',
                env=env,
                bufsize=1
            )

            for line in process.stdout:
                line = line.strip()
                if not line: continue

                # Debug
                if "ERROR:" in line or "WARNING:" in line:
                    error_log.append(line)
                    print(f"[YT-DLP LOG] {line}")

                if "[download]" in line and "%" in line:
                    parts = line.split()
                    for p in parts:
                        if "%" in p:
                            if progress_callback:
                                progress_callback(p, "Baixando...")
                            break
                elif "[Merger]" in line:
                    if progress_callback: progress_callback("99%", "Unindo Áudio/Vídeo...")
                    print("[Downloader] Unindo arquivos...")
                elif "Destination:" in line:
                    print(f"[Downloader] Arquivo: {line}")

            process.wait()

            if process.returncode == 0:
                if progress_callback: progress_callback("100%", "Concluído ✅")
                print(f"[Downloader] Sucesso: {title}")
                return True
            else:
                print("\n" + "="*30)
                print("❌ ERRO FATAL NO YT-DLP")
                for err in error_log: print(f" > {err}")
                print("="*30 + "\n")

                msg_erro = "Erro no Download"
                if any("ffmpeg" in e.lower() for e in error_log):
                    msg_erro = "Falta FFmpeg (Vídeo sem som)"
                elif "requested format is not available" in str(error_log).lower():
                     msg_erro = "Formato indisponível (Tente outro)"

                if progress_callback: progress_callback("0%", msg_erro)
                return False

        except Exception as e:
            print(f"[Downloader Exception] {e}")
            if progress_callback: progress_callback("0%", "Erro Crítico")
            return False
