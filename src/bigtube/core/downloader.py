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
            import traceback
            traceback.print_exc()
            print(f"[Erro Fatal] {e}")
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

        # DEBUG: Ver quantos formatos chegaram
        print(f"[Downloader] Formatos encontrados: {len(formats)}")

        for f in formats:
            # Filtros básicos de lixo
            format_note = f.get('format_note') or ''
            protocol = f.get('protocol') or ''

            if 'storyboard' in format_note or protocol == 'http_dash_segments':
                continue

            fmt_id = str(f.get('format_id') or '')
            ext = f.get('ext')
            print(ext)

            # Pega valores brutos (podem ser None)
            vcodec = str(f.get('vcodec') or 'none').split('.')[0]
            acodec = str(f.get('acodec') or 'none').split('.')[0]

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
            is_audio_only = (
                vcodec == 'none' or vcodec is None
            ) and (
                acodec != 'none' and acodec is not None
            )

            # É VÍDEO SE: tem altura definida (height > 0)
            height = f.get('height') or -1
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
                    'quality': abr,
                    'codec': acodec
                })

                clean_data['audios'] = self._del_duplic(clean_data['audios'])
                # Ordena Áudios: Qualidade > Tamanho
                clean_data['audios'].sort(
                    key=lambda x: (x['quality'], x['size_val']),
                    reverse=True
                )

            # --- 2. PROCESSAR VÍDEO ---
            elif is_video:
                fps = f.get('fps') or 0

                # Monta Label
                label_parts = [f"{height}p"]
                if fps > 30:
                    label_parts.append(f"{int(fps)}fps")
                label_parts.append(f"({ext})")

                # Codec Info
                vc = str(vcodec).lower()
                if 'av01' in vc:
                    label_parts.append("[AV1]")
                elif 'vp9' in vc:
                    label_parts.append("[VP9]")
                elif 'avc1' in vc or 'h264' in vc:
                    label_parts.append("[H.264]")
                else:
                    label_parts.append(f"[{vc.upper()}]")

                if f.get('dynamic_range') == 'HDR':
                    label_parts.append("HDR")

                clean_data['videos'].append({
                    'id': fmt_id,
                    'label': " ".join(label_parts),
                    'resolution': height,
                    'fps': fps,
                    'ext': ext,
                    'size': size_str,
                    'size_val': size_mb,
                    'type': 'video',
                    'codec': vc
                })

                # --- ORDENAÇÃO E LIMPEZA ---

                # Remove duplicatas exatas de label para limpar a lista visual
                clean_data['videos'] = self._del_duplic(clean_data['videos'])
                # Ordena Vídeos: Resolução > FPS > Tamanho
                clean_data['videos'].sort(
                    key=lambda x: (x['resolution'], x['fps'], x['size_val']),
                    reverse=True
                )

        return clean_data

    def _del_duplic(self, items):
        seen = set()
        unique = []
        for item in items:
            # Chave única: Label + Extensão + Tamanho Aprox
            key = (item['label'], item['ext'], int(item['size_val']))
            if key not in seen:
                unique.append(item)
                seen.add(key)
        return unique

    def download_video(self, url, format_id, title, ext, progress_callback=None):
        """
        Baixa o vídeo/áudio com progresso em tempo real e suporte a SoundCloud.
        """
        print(f"[Downloader] Iniciando: {title}")

        self.download_folder = Config.get("download_path")
        if not os.path.exists(self.download_folder):
            os.makedirs(self.download_folder)

        # 1. Sanitização de Nome (Mantive sua lógica, é boa)
        safe_title = "".join([c for c in title if c.isalnum() or c in " -_()."]).strip()
        if not safe_title:
            safe_title = f"video_{format_id}"

        output_template = os.path.join(
            self.download_folder,
            f"{safe_title}.%(ext)s"
        )

        # 2. LÓGICA DE COMANDO DINÂMICA
        cmd = [
            self.binary,
            "--no-warnings",
            "--newline",
            "--no-playlist",
            "--ignore-config",
            "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.212 Safari/537.36",
            "-o", output_template,
        ]

        # Verifica se é modo ÁUDIO (MP3, WAV, M4A)
        is_audio_mode = ext in ['mp3', 'wav', 'm4a', 'opus', 'flac']

        if is_audio_mode:
            cmd.extend([
                "-f", f"{format_id}",
                "--extract-audio",
                "--audio-format", ext,
                "--audio-quality", "0",
            ])
        else:
            if "+bestaudio" not in format_id:
                cmd.extend(["-f", f"{format_id}+bestaudio/best"])
            else:
                cmd.extend(["-f", format_id])

            # Só usa merge se for vídeo
            cmd.extend(["--merge-output-format", ext])

        cmd.append(url)

        env = os.environ.copy()
        env["PATH"] = str(Config.BIN_DIR) + os.pathsep + env.get("PATH", "")

        error_log = []

        try:
            # 3. EXECUÇÃO COM PIPE EM TEMPO REAL
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
                if not line:
                    continue

                # Debug no console (Opcional, pode comentar em produção)
                print(f"[YT-DLP] {line}")

                # Captura erros para log
                if "ERROR:" in line or "WARNING:" in line:
                    error_log.append(line)

                # --- PARSING DE PROGRESSO ---
                if "[download]" in line and "%" in line:
                    parts = line.split()
                    for p in parts:
                        if "%" in p:
                            # Remove cores ANSI se houver e pega a porcentagem
                            percent = p.replace('%', '')
                            # Chama o callback (UI Update)
                            if progress_callback:
                                progress_callback(f"{percent}%", "Baixando...")
                            break

                elif "[Merger]" in line or "[ExtractAudio]" in line:
                    if progress_callback:
                        progress_callback("99%", "Processando Áudio...")

                elif "[Fixup]" in line:
                    if progress_callback:
                        progress_callback("99%", "Finalizando arquivo...")

            # Aguarda o fim do processo
            process.wait()

            if process.returncode == 0:
                if progress_callback:
                    progress_callback("100%", "Concluído ✅")
                print(f"[Downloader] Sucesso: {safe_title}")
                return True
            else:
                # TRATAMENTO DE ERROS INTELIGENTE
                print(f"ERRO FATAL (Code {process.returncode})")

                msg_erro = "Erro desconhecido"
                if any("ffmpeg" in e.lower() for e in error_log):
                    msg_erro = "Erro: FFmpeg não instalado"
                elif any("sign" in e.lower() for e in error_log):
                    msg_erro = "Erro: Assinatura/Bloqueio do YouTube"
                elif "invalid merge" in str(error_log).lower():
                    msg_erro = "Erro interno de formato"

                if progress_callback:
                    progress_callback("Erro", msg_erro)

                return False

        except Exception as e:
            print(f"[Downloader Exception] {e}")
            if progress_callback:
                progress_callback("Erro", "Falha Crítica no Sistema")
            return False
