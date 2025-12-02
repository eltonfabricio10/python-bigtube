# -*- coding: utf-8 -*-
import os
import sys
import json
from pathlib import Path


class Config:
    # Pasta Base
    APP_DATA_DIR = Path(os.path.expanduser("~/.local/share/bigtube"))
    BIN_DIR = APP_DATA_DIR / "bin"
    CONFIG_FILE = APP_DATA_DIR / "config.json"

    YT_DLP_NAME = "yt-dlp"
    DENO_NAME = "deno"

    YT_DLP_PATH = BIN_DIR / YT_DLP_NAME
    DENO_PATH = BIN_DIR / DENO_NAME

    DATA = {
        "download_path": os.path.join(os.path.expanduser("~"), "Downloads", "BigTube"),
        "theme": "dark"
    }

    @staticmethod
    def ensure_dirs():
        Config.BIN_DIR.mkdir(parents=True, exist_ok=True)
        # Se não existe config, cria
        Config.load()

    @staticmethod
    def load():
        """Lê o JSON. Se vazio ou corrompido, reseta para o padrão."""
        if not Config.CONFIG_FILE.exists():
            print("[Config] Arquivo não existe. Criando padrão...")
            Config.save()
            return

        try:
            with open(Config.CONFIG_FILE, 'r', encoding='utf-8') as f:
                content = f.read().strip()

                # Se arquivo estiver vazio fisicamente
                if not content:
                    raise ValueError("Arquivo vazio")

                saved = json.loads(content)
                Config.DATA.update(saved)
                # print(f"[Config] Carregado: {Config.DATA}")

        except (json.JSONDecodeError, ValueError, Exception) as e:
            print(f"[Config] Erro crítico ao ler ({e}). Resetando config para padrão.")
            # Auto-Recuperação: Sobrescreve o arquivo ruim com o padrão da memória
            Config.save()

    @staticmethod
    def save():
        """Salva o estado atual da memória no disco."""
        try:
            with open(Config.CONFIG_FILE, 'w', encoding='utf-8') as f:
                json.dump(Config.DATA, f, indent=4)
            print("[Config] Salvo com sucesso.")
        except Exception as e:
            print(f"[Config] Erro ao salvar: {e}")

    @staticmethod
    def get(key):
        return Config.DATA.get(key)

    @staticmethod
    def set(key, value):
        Config.DATA[key] = value
        Config.save()
