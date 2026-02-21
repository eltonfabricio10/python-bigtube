#!/usr/bin/env python3
import os
import subprocess
from pathlib import Path

# Configuration
APP_NAME = "bigtube"
PROJECT_ROOT = Path(__file__).parent.parent
SRC_DIR = PROJECT_ROOT / "src" / "bigtube"
LOCALE_DIR = PROJECT_ROOT / "locales"
POT_FILE = LOCALE_DIR / f"{APP_NAME}.pot"

def run_command(command):
    print(f"Running: {' '.join(command)}")
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error: {result.stderr}")
    return result.returncode == 0

def main():
    # Ensure locales directory exists
    os.makedirs(LOCALE_DIR, exist_ok=True)

    print(f"Extracting translations for {APP_NAME}...")

    # Find Python files
    py_files = []
    for root, _, files in os.walk(SRC_DIR):
        for file in files:
            if file.endswith(".py"):
                py_files.append(str(Path(root) / file))

    # Find UI files
    ui_files = []
    for root, _, files in os.walk(SRC_DIR):
        for file in files:
            if file.endswith(".ui"):
                ui_files.append(str(Path(root) / file))

    if not py_files and not ui_files:
        print("No files found to extract strings from.")
        return

    # 1. Extract from Python files
    # We use -k_ and -kN_ to match the functions used in locales.py
    # --from-code=UTF-8 ensures correct handling of non-ASCII characters
    python_cmd = [
        "xgettext",
        "--language=Python",
        "--from-code=UTF-8",
        f"--output={POT_FILE}",
        "--keyword=_",
        "--keyword=N_",
        "--add-comments=TRANSLATORS:",
        "--package-name=BigTube",
    ] + py_files

    if run_command(python_cmd):
        print(f"Strings extracted from {len(py_files)} Python files.")
    else:
        print("Failed to extract strings from Python files.")
        return

    # 2. Extract from UI files (GTK/Glade format)
    # We append to the existing POT file using -j (join)
    if ui_files:
        ui_cmd = [
            "xgettext",
            "--language=Glade",
            "--from-code=UTF-8",
            f"--output={POT_FILE}",
            "--join-existing",
            "--add-comments=TRANSLATORS:",
        ] + ui_files

        if run_command(ui_cmd):
            print(f"Strings extracted from {len(ui_files)} UI files.")
        else:
            print("Failed to extract strings from UI files.")

    print(f"\nSuccessfully generated: {POT_FILE}")
    print("You can now use this .pot file to create or update .po files for specific languages.")

if __name__ == "__main__":
    main()
