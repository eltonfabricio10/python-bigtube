# Guia para Desenvolvedores

Este documento descreve a estrutura do projeto, como compilar, rodar testes e lint, e convenções para contribuir ao BigTube.

> O BigTube foi portado de Python para **Rust**. Todo o código ativo vive em `rust/`.
> O histórico do port está documentado em [PORTING_RUST.md](PORTING_RUST.md).

## Pré-requisitos

- **Rust** estável (via [rustup](https://rustup.rs/))
- Bibliotecas de sistema: **GTK4 ≥ 4.12**, **libadwaita ≥ 1.5**, **GStreamer** (base/good/bad + `gst-plugin-gtk4`)
- **gettext** (`msgfmt`) para compilar as traduções
- Em tempo de execução: **yt-dlp**; opcionalmente **ffmpeg** para conversão

## Configuração do ambiente

```bash
git clone https://github.com/eltonfabricio10/bigtube.git
cd bigtube/rust
cargo build
```

## Executando a aplicação

```bash
cd rust
cargo run -p bigtube-gui      # interface gráfica
cargo run -p bigtube-cli -- --help   # linha de comando
```

Variáveis de ambiente úteis (veja a seção "Variáveis de ambiente" no [README.md](README.md)):

- `RUST_LOG=debug` — logs detalhados (via `tracing`)
- `BIGTUBE_NO_FULL_REDRAW=1` — desativa o full-redraw do GSK (workaround de glitch de scroll)
- `GSK_RENDERER` — força um renderizador GSK específico

## Arquitetura do código

O workspace Cargo tem três crates em `rust/crates/`:

| Crate          | Responsabilidade |
|----------------|------------------|
| `bigtube-core` | Lógica de negócio testável e headless: download (yt-dlp), conversão (ffmpeg), config, histórico, agendamento, rede, validators |
| `bigtube-cli`  | Front-end de linha de comando sobre o core |
| `bigtube-gui`  | Interface GTK4/libadwaita sobre o core (janela, páginas de busca/downloads/conversor, player, diálogos) |

Os assets do GUI (`style.css`) ficam em `rust/crates/bigtube-gui/assets/` e são embutidos no binário via `include_str!`. Os ícones do app ficam em `assets/` (raiz do repo).

Tarefas pesadas rodam em threads de background; atualizações de UI voltam ao main loop do GTK via `glib::idle_add`/canais. Erros de UI são tratados de forma amigável (toasts/status) e logados com `tracing`.

## Testes

A suíte de testes cobre o **core** (sem display):

```bash
cd rust
cargo test -p bigtube-core
```

## Lint e formatação

A CI exige `rustfmt` e `clippy` limpos:

```bash
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Rode `cargo fmt --all` antes de commitar.

## Versão automática

A versão deriva do Git. O script [`scripts/sync_version_from_git.py`](scripts/sync_version_from_git.py) calcula a versão (`git describe`), e [`scripts/sync_rust_version.py`](scripts/sync_rust_version.py) — usado pelo workflow de release — sincroniza `rust/Cargo.toml`, `rust/Cargo.lock` e `rust/packaging/PKGBUILD`. O binário também expõe a versão derivada do Git em tempo de build via `bigtube-cli/build.rs`.

**Release oficial:** o workflow **Build and Release** dispara quando o **Rust CI** da `main` passa — ele cria a tag `vX.Y.Z`, compila os binários e publica a GitHub Release (tarball Arch + `.deb` + `.rpm`) e atualiza o pacote `bigtube-bin` no AUR.

## Traduções

Os catálogos ficam em `po/` (`.po` + `bigtube.pot`). Para testar uma tradução localmente, compile o `.mo` para o diretório de override por usuário:

```bash
mkdir -p ~/.local/share/locale/pt_BR/LC_MESSAGES
msgfmt po/pt_BR.po -o ~/.local/share/locale/pt_BR/LC_MESSAGES/bigtube.mo
```

O script `scripts/auto_translate.py` (polib + deep-translator) pode auxiliar na geração/atualização das traduções. O empacotamento (`rust/packaging/stage-tree.sh`) compila todos os `.po` para `/usr/share/locale` automaticamente.

## CI

O workflow **Rust CI** (`.github/workflows/rust-ci.yml`) roda em cada push/PR para `main`/`master`:

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test -p bigtube-core`
4. `cargo build --release`

Garanta que esses passos passem localmente antes de enviar o PR.

## Dúvidas

Para uso e instalação, veja o [README.md](README.md). Para discussão de desenvolvimento, abra uma issue no repositório.
