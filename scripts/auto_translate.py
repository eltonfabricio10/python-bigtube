#!/usr/bin/env python3
"""
This script automates the translation of the project's source code
and UI files into multiple languages using the Google Translate API.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

# =========================================================================
# Dependency Check
# =========================================================================
try:
    import polib
    from deep_translator import GoogleTranslator
except ImportError:
    print("[auto_translate] Error: Translation dependencies not found.")
    print("Please install them by running: poetry add polib deep-translator --group dev")
    sys.exit(1)

# =========================================================================
# Project Configuration
# =========================================================================
APP_NAME = "bigtube"
# Assuming this script is located inside the 'scripts/' folder at the project root
PROJECT_ROOT = Path(__file__).parent.parent
SRC_DIR = PROJECT_ROOT / "src" / APP_NAME
PO_DIR = PROJECT_ROOT / "po"
POT_FILE = PO_DIR / f"{APP_NAME}.pot"

# Mapping of standard Linux locale codes to Google Translate API language codes.
# Any locale added here will be AUTOMATICALLY CREATED if it doesn't exist.
LANG_MAP = {
    # ==========================================
    # Americas & Iberian Peninsula
    # ==========================================
    'en_US': 'en',     # English (USA)
    'pt_BR': 'pt',     # Portuguese (Brazil)
    'pt_PT': 'pt',     # Portuguese (Portugal)
    'es_ES': 'es',     # Spanish (Spain)
    'es_MX': 'es',     # Spanish (Mexico)
    'es_AR': 'es',     # Spanish (Argentina)

    # ==========================================
    # Western & Central Europe
    # ==========================================
    'fr_FR': 'fr',     # French
    'de_DE': 'de',     # German
    'it_IT': 'it',     # Italian
    'nl_NL': 'nl',     # Dutch
    'pl_PL': 'pl',     # Polish
    'cs_CZ': 'cs',     # Czech
    'hu_HU': 'hu',     # Hungarian
    'ro_RO': 'ro',     # Romanian
    'sk_SK': 'sk',     # Slovak

#    # ==========================================
#    # Nordic Countries
#    # ==========================================
#    'sv_SE': 'sv',     # Swedish
#    'da_DK': 'da',     # Danish
#    'no_NO': 'no',     # Norwegian
#    'fi_FI': 'fi',     # Finnish
#
#    # ==========================================
#    # Eastern Europe & Balkans
#    # ==========================================
#    'ru_RU': 'ru',     # Russian
#    'uk_UA': 'uk',     # Ukrainian
#    'bg_BG': 'bg',     # Bulgarian
#    'el_GR': 'el',     # Greek
#    'hr_HR': 'hr',     # Croatian
#    'sr_RS': 'sr',     # Serbian
#
#    # ==========================================
#    # Asia & Pacific
#    # ==========================================
#    'zh_CN': 'zh-CN',  # Chinese (Simplified)
#    'zh_TW': 'zh-TW',  # Chinese (Traditional)
#    'ja_JP': 'ja',     # Japanese
#    'ko_KR': 'ko',     # Korean
#    'vi_VN': 'vi',     # Vietnamese
#    'th_TH': 'th',     # Thai
#    'id_ID': 'id',     # Indonesian
#    'ms_MY': 'ms',     # Malay
#    'tl_PH': 'tl',     # Tagalog / Filipino
#
#    # ==========================================
#    # Middle East, Africa & India
#    # ==========================================
#    'ar_AE': 'ar',     # Arabic (UAE)
#    'ar_SA': 'ar',     # Arabic (Saudi Arabia)
#    'he_IL': 'he',     # Hebrew
#    'tr_TR': 'tr',     # Turkish
#    'fa_IR': 'fa',     # Persian
#    'hi_IN': 'hi',     # Hindi
#    'bn_IN': 'bn',     # Bengali
#    'ur_PK': 'ur',     # Urdu
#    'sw_KE': 'sw'      # Swahili
}

# =========================================================================
# Helper Functions
# =========================================================================
def run_command(command, quiet=False):
    """Executes a shell command and returns True if successful."""
    if not quiet:
        print(f"[auto_translate] Running: {' '.join(command)}")
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if result.returncode != 0 and not quiet:
        print(f"[auto_translate] Error: {result.stderr}")
    return result.returncode == 0

def protect_placeholders(text):
    """
    Protects Python format variables (e.g., {count}) from being translated
    by replacing them with temporary safe tokens.
    """
    placeholders = re.findall(r'\{.*?\}', text)
    safe_text = text
    for i, p in enumerate(placeholders):
        safe_text = safe_text.replace(p, f'__TOKEN{i}__')
    return safe_text, placeholders

def restore_placeholders(text, placeholders):
    """Restores the original Python format variables into the translated string."""
    for i, p in enumerate(placeholders):
        text = text.replace(f'__TOKEN{i}__', p)
    return text

def translate_po_file(file_path, target_lang_code):
    """
    Parses a .po file, finds empty translations, translates them via Google API,
    and saves the file back to disk.
    """
    print(f"\n [auto_translate] Translating new strings in: {file_path.name} (Target: {target_lang_code})")

    try:
        po = polib.pofile(str(file_path))
        translator = GoogleTranslator(source='en', target=target_lang_code)

        translated_count = 0

        for entry in po:
            # Translate ONLY if the string is empty and not obsolete
            if not entry.msgstr and not entry.obsolete:
                original_text = entry.msgid

                # 1. Protect Python variables
                safe_text, placeholders = protect_placeholders(original_text)

                try:
                    # 2. Translate via API
                    translated_safe = translator.translate(safe_text)

                    # 3. Restore variables
                    final_translation = restore_placeholders(translated_safe, placeholders)

                    # 4. FIX: Preserve trailing and leading newlines for msgfmt strictness
                    if original_text.endswith('\n') and not final_translation.endswith('\n'):
                        final_translation += '\n'
                    if original_text.startswith('\n') and not final_translation.startswith('\n'):
                        final_translation = '\n' + final_translation

                    entry.msgstr = final_translation
                    translated_count += 1

                    print(f"[auto_translate] '{original_text}' -> '{final_translation}'")

                except Exception as e:
                    print(f"[auto_translate] API Error translating '{original_text}': {e}")

        # Save changes if any new translation was made
        if translated_count > 0:
            po.save(str(file_path))
            print(f"[auto_translate] Success! {translated_count} new strings translated and saved.")
        else:
            print("[auto_translate] No new strings found. File is already 100% translated.")

    except Exception as e:
        print(f"[auto_translate] Fatal error processing {file_path.name}: {e}")

# =========================================================================
# Main Workflow (Extract -> Init -> Merge -> Translate)
# =========================================================================
def main():
    """
    Main workflow for i18n automation.
    This function orchestrates the entire i18n process:
    - Collects source files
    - Extracts strings using xgettext
    - Initializes new language files using msginit
    - Merges new strings into existing .po files
    - Translates new strings using the Google Translate API
    - Saves the translated .po files back to disk
    """
    print("[auto_translate] Starting i18n Automation")
    print(f"[auto_translate] Project: {APP_NAME}")
    print(f"[auto_translate] Source Directory: {SRC_DIR}")
    print(f"[auto_translate] PO Directory: {PO_DIR}")
    print(f"[auto_translate] Language Map: {LANG_MAP}")
    print(f"[auto_translate] POT File: {POT_FILE}")

    # Create PO directory if it doesn't exist
    os.makedirs(PO_DIR, exist_ok=True)

    # --- PHASE 1: Collect Source Files ---
    py_files, ui_files = [], []
    for root, _, files in os.walk(SRC_DIR):
        for file in files:
            if file.endswith(".py"):
                py_files.append(str(Path(root) / file))
            elif file.endswith(".ui"):
                ui_files.append(str(Path(root) / file))

    if not py_files and not ui_files:
        print("[auto_translate] No source files found for string extraction.")
        return

    # --- PHASE 2: Extraction (xgettext) ---
    print("\n [auto_translate] PHASE 1: Extracting strings from source code...")
    if py_files:
        python_cmd = [
            "xgettext", "--language=Python", "--from-code=UTF-8",
            f"--output={POT_FILE}", "--keyword=_", "--keyword=N_",
            "--add-comments=TRANSLATORS:", f"--package-name={APP_NAME}"
        ] + py_files
        run_command(python_cmd)

    if ui_files:
        ui_cmd = [
            "xgettext", "--language=Glade", "--from-code=UTF-8",
            f"--output={POT_FILE}", "--join-existing",
            "--add-comments=TRANSLATORS:"
        ] + ui_files
        run_command(ui_cmd)

    if not POT_FILE.exists():
        print("[auto_translate] Error: .pot file was not generated.")
        return

    # --- PHASE 2.5: Initialize Missing Languages (msginit) ---
    print("\n [auto_translate] PHASE 2: Verifying and creating missing languages...")
    for lang_code in LANG_MAP.keys():
        po_path = PO_DIR / f"{lang_code}.po"
        if not po_path.exists():
            print(f"  [auto_translate] [+] Creating new language file: {lang_code}.po")
            # Uses msginit without interactive prompts
            init_cmd = [
                "msginit",
                "--no-translator",
                f"--input={POT_FILE}",
                f"--locale={lang_code}",
                f"--output={po_path}"
            ]
            run_command(init_cmd, quiet=True)

    # --- PHASE 3: Merge (msgmerge) ---
    print("\n [auto_translate] PHASE 3: Updating existing .po files...")
    po_files = [f for f in os.listdir(PO_DIR) if f.endswith(".po")]

    for file in po_files:
        po_path = PO_DIR / file
        merge_cmd = ["msgmerge", "--update", "--backup=none", str(po_path), str(POT_FILE)]
        run_command(merge_cmd, quiet=True)
        print(f"  [auto_translate] [+] Synced: {file}")

    # --- PHASE 4: Auto Translation ---
    print("\n [auto_translate] PHASE 4: Starting Auto-Translation Engine...")
    for file in po_files:
        po_path = PO_DIR / file
        lang_id = file.replace(".po", "")

        target_lang = LANG_MAP.get(lang_id, lang_id.split('_')[0])
        translate_po_file(po_path, target_lang)

    print("\n [auto_translate] Process Complete! Your codebase is synced and translated.")

if __name__ == "__main__":
    main()
