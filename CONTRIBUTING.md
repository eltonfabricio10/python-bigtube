# Guia para Desenvolvedores

Este documento descreve a estrutura do projeto, como rodar testes e lint, e convenções para contribuir ao BigTube.

## Pré-requisitos

- **Python 3.10+**
- **Poetry** ([instalação](https://python-poetry.org/docs/#installation))
- Para rodar a aplicação com interface: GTK4, Libadwaita, GStreamer (dependências do sistema)

## Configuração do ambiente

```bash
git clone https://github.com/eltonfabricio10/python-bigtube.git
cd python-bigtube
poetry install
```

Para incluir dependências de desenvolvimento (pytest, ruff, etc.):

```bash
poetry install --with dev
```

## Executando a aplicação

```bash
poetry run bigtube
```

Opções úteis:

- `--debug` — ativa logs detalhados
- `--version` — exibe a versão do yt-dlp

## Arquitetura do código

O código principal está em `src/bigtube/`:

| Diretório     | Responsabilidade |
|---------------|------------------|
| `core/`       | Lógica de negócio: download (yt-dlp), conversão (ffmpeg), config, histórico, logger, rede, clipboard, i18n |
| `controllers/`| Orquestração entre UI e core: busca, downloads, conversor, configurações, player |
| `ui/`         | Widgets e janelas GTK4/Libadwaita: janela principal, linhas de download/busca/conversor, player, diálogos |
| `data/`       | Recursos: templates `.ui`, CSS, arquivo `.desktop` |

### Fluxos principais

- **Download**: usuário busca ou cola URL → `SearchController` / main window → `VideoDownloader.fetch_video_info` → diálogo de formato → `DownloadManager.schedule_download` → `VideoDownloader` em thread → atualização de UI via `GLib.idle_add`.
- **Conversão**: lista de arquivos no `ConverterController` → `MediaConverter` (ffmpeg) em thread → callbacks de progresso e conclusão via `GLib.idle_add`.
- **Player**: `PlayerController` + `VideoWindow` com widget MPV/GStreamer; “play next/prev” integrado ao histórico de downloads.

Tarefas pesadas rodam em `threading.Thread`; atualizações de interface devem ser feitas no main loop do GTK via `GLib.idle_add()`. O módulo **`ui/async_utils.py`** oferece `run_in_background(fn, on_success=..., on_error=...)` para centralizar esse padrão (exemplo de uso em `SearchController`).

## Testes

A suíte usa **pytest**. Os testes mockam GTK e dependências externas para rodar sem display.

```bash
poetry run pytest
```

Opções úteis:

```bash
poetry run pytest tests/ -v              # verboso
poetry run pytest tests/ -k "download"  # só testes com "download" no nome
poetry run pytest tests/ --tb=long      # traceback completo
```

Testes cobrem sobretudo o **core** (downloader, conversor, config, histórico, validators, scheduler, clipboard). A UI e os controllers têm pouca cobertura automatizada.

## Lint e formatação

O projeto usa **Ruff** para lint e formatação.

- **Verificar problemas (lint):**
  ```bash
  poetry run ruff check .
  ```

- **Verificar formatação (sem alterar arquivos):**
  ```bash
  poetry run ruff format --check .
  ```

- **Corrigir automaticamente** (quando possível) e **formatar:**
  ```bash
  poetry run ruff check . --fix
  poetry run ruff format .
  ```

Recomenda-se rodar `ruff check` e `ruff format` antes de abrir um pull request.

## Convenções de código

- **Estilo**: Ruff (pycodestyle, isort, pyupgrade). Linha máxima 100 caracteres; aspas duplas para strings.
- **Imports**: ordem isort (stdlib → third-party → local). Ruff aplica isso.
- **Strings de interface**: usar o sistema de i18n em `core/locales.py` (`ResourceManager`, `StringKey`) em vez de strings fixas na UI.
- **Threads + UI**: não atualizar widgets GTK a partir de threads; usar `GLib.idle_add(callback, ...)` para voltar ao main loop.

## Traduções

Arquivos de tradução ficam em `po/`. Para compilar após alterar `.po`:

```bash
# Exemplo: compilar para pt_BR
mkdir -p src/bigtube/data/locales/pt_BR/LC_MESSAGES
msgfmt po/pt_BR.po -o src/bigtube/data/locales/pt_BR/LC_MESSAGES/bigtube.mo
```

O script `scripts/auto_translate.py` pode auxiliar na geração/atualização de traduções (polib + deep-translator).

## CI

O workflow **CI** (`.github/workflows/ci.yml`) roda em cada push/PR para os branches `main` e `master`:

1. Instala dependências com Poetry (incluindo dev).
2. Executa `ruff check` e `ruff format --check`.
3. Executa `pytest`.

Garanta que esses comandos passem localmente antes de enviar o PR.

## Dúvidas

Para funcionalidades de usuário e instalação, veja o [README.md](README.md). Para discussão de desenvolvimento e contribuição, abra uma issue no repositório.
