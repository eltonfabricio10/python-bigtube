# -*- coding: utf-8 -*-
import os
import stat
import urllib.request
import subprocess
import zipfile
import io
from .config import Config


class Updater:
    @staticmethod
    def get_local_version():
        """Pergunta a versão para o binário local via linha de comando."""
        if not Config.YT_DLP_PATH.exists():
            return None

        try:
            # Executa: ./yt-dlp --version
            result = subprocess.run(
                [str(Config.YT_DLP_PATH), "--version"],
                capture_output=True, text=True
            )
            return result.stdout.strip()
        except Exception:
            return "Erro"

    @staticmethod
    def update_yt_dlp():
        """
        Baixa a versão mais recente direto do GitHub oficial.
        """
        Config.ensure_dirs()
        print(f"[Updater] Baixando yt-dlp em: {Config.YT_DLP_PATH}")

        # URL oficial do binário Linux
        url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"

        try:
            # 1. Baixa o arquivo
            print("[Updater] Download iniciado...")
            with urllib.request.urlopen(url) as response, open(Config.YT_DLP_PATH, 'wb') as out_file:
                out_file.write(response.read())

            # 2. Dá permissão de execução (chmod +x)
            st = os.stat(Config.YT_DLP_PATH)
            os.chmod(Config.YT_DLP_PATH, st.st_mode | stat.S_IEXEC)

            print("[Updater] Binário instalado e executável!")
            return True, Updater.get_local_version()

        except Exception as e:
            print(f"[Updater] Erro crítico: {e}")
            return False, str(e)

    @staticmethod
    def update_deno():
        """
        Baixa o Deno (Runtime JS portátil).
        """
        print(f"[Updater] Baixando Deno para: {Config.DENO_PATH}")

        # URL oficial para Linux x64 (Ajustar se for ARM/Mac)
        url = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"

        try:
            # 1. Baixa o ZIP na memória
            with urllib.request.urlopen(url) as response:
                zip_data = response.read()

            # 2. Extrai apenas o binário 'deno'
            with zipfile.ZipFile(io.BytesIO(zip_data)) as z:
                # O zip contem um arquivo chamado 'deno'
                with z.open("deno") as zf, open(Config.DENO_PATH, 'wb') as f:
                    f.write(zf.read())

            # 3. Dá permissão de execução
            st = os.stat(Config.DENO_PATH)
            os.chmod(Config.DENO_PATH, st.st_mode | stat.S_IEXEC)

            print("[Updater] Deno instalado com sucesso!")
            return True

        except Exception as e:
            print(f"[Updater Error] Falha ao baixar Deno: {e}")
            return False

    @staticmethod
    def ensure_exists():
        """Verifica se yt-dlp E Deno existem. Se não, baixa."""
        Config.ensure_dirs()

        # 1. Garante yt-dlp
        if not Config.YT_DLP_PATH.exists():
            print("[System] yt-dlp ausente. Baixando...")
            Updater.update_yt_dlp()

        # 2. Garante Deno (Runtime JS)
        if not Config.DENO_PATH.exists():
            print("[System] Runtime JS (Deno) ausente. Baixando...")
            Updater.update_deno()
