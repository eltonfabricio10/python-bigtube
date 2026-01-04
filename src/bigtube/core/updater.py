import os
import stat
import urllib.request
import subprocess
import zipfile
import io
import sys
from typing import Optional, Tuple
from pathlib import Path

# Internal Imports
from .config import ConfigManager


class Updater:
    """
    Handles automatic downloading and updating of external binaries
    (yt-dlp and Deno) required for the application to function.
    """

    # Direct download URLs
    _YT_DLP_URL = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
    _DENO_URL = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip"

    @staticmethod
    def get_local_version() -> Optional[str]:
        """
        Queries the local yt-dlp binary for its version.
        Returns the version string (e.g., '2023.10.13') or None if missing/error.
        """
        binary_path = ConfigManager.YT_DLP_PATH

        if not binary_path.exists():
            return None

        try:
            # Exec: ./yt-dlp --version
            result = subprocess.run(
                [str(binary_path), "--version"],
                capture_output=True,
                text=True,
                check=False
            )
            if result.returncode == 0:
                return result.stdout.strip()
            return "Unknown"
        except Exception as e:
            print(f"[Updater] Error checking version: {e}")
            return "Error"

    @staticmethod
    def update_yt_dlp() -> Tuple[bool, str]:
        """
        Downloads the latest yt-dlp binary from GitHub.
        Returns: (Success: bool, Version/Error: str)
        """
        ConfigManager.ensure_dirs()
        target_path = ConfigManager.YT_DLP_PATH

        print(f"[Updater] Downloading yt-dlp to: {target_path}")

        try:
            # 1. Download stream
            with urllib.request.urlopen(Updater._YT_DLP_URL, timeout=30) as response:
                data = response.read()

            # 2. Write to file
            with open(target_path, 'wb') as out_file:
                out_file.write(data)

            # 3. Grant Execution Permissions (chmod +x)
            st = os.stat(target_path)
            os.chmod(target_path, st.st_mode | stat.S_IEXEC)

            new_version = Updater.get_local_version()
            print(f"[Updater] yt-dlp installed successfully! Version: {new_version}")
            return True, str(new_version)

        except Exception as e:
            print(f"[Updater] Critical Error updating yt-dlp: {e}")
            return False, str(e)

    @staticmethod
    def update_deno() -> bool:
        """
        Downloads and extracts the Deno runtime (required for some JS signatures).
        """
        ConfigManager.ensure_dirs()
        target_path = ConfigManager.DENO_PATH

        print(f"[Updater] Downloading Deno to: {target_path}")

        try:
            # 1. Download ZIP to memory
            with urllib.request.urlopen(Updater._DENO_URL, timeout=60) as response:
                zip_data = response.read()

            # 2. Extract specific binary
            with zipfile.ZipFile(io.BytesIO(zip_data)) as z:
                # The zip contains a file named 'deno' (no extension on Linux)
                if "deno" in z.namelist():
                    with z.open("deno") as zf, open(target_path, 'wb') as f:
                        f.write(zf.read())
                else:
                    raise FileNotFoundError("deno binary not found in downloaded zip")

            # 3. Grant Execution Permissions
            st = os.stat(target_path)
            os.chmod(target_path, st.st_mode | stat.S_IEXEC)

            print("[Updater] Deno installed successfully!")
            return True

        except Exception as e:
            print(f"[Updater] Failed to download Deno: {e}")
            return False

    @staticmethod
    def ensure_exists():
        """
        Checks if required binaries exist. Downloads them if missing.
        Blocking call - should ideally be run in a thread or splash screen.
        """
        ConfigManager.ensure_dirs()

        # 1. Check yt-dlp
        if not ConfigManager.YT_DLP_PATH.exists():
            print("[System] yt-dlp missing. Starting auto-download...")
            Updater.update_yt_dlp()

        # 2. Check Deno
        if not ConfigManager.DENO_PATH.exists():
            print("[System] JS Runtime (Deno) missing. Starting auto-download...")
            Updater.update_deno()
