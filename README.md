<p align="center">
  <img src="https://raw.githubusercontent.com/eltonfabricio10/python-bigtube/main/assets/banner.png" alt="BigTube Banner" width="100%">
</p>

# 🎬 BigTube

> **O Downloader Multimídia Definitivo para Linux**

**BigTube** é uma aplicação desktop moderna, veloz e elegante, construída em **Rust** com **GTK4**, **Libadwaita** e **GStreamer**. Projetado para quem não aceita menos que a perfeição ao baixar conteúdos da internet, o BigTube transforma a complexidade do `yt-dlp` em uma ferramenta intuitiva e poderosa — agora como um binário nativo, sem dependências de runtime Python.

> ℹ️ A partir da versão **2.0**, o BigTube foi reescrito em Rust. O pacote AUR recomendado passou a ser **`bigtube-bin`** (binário pré-compilado). O antigo pacote `bigtube` (Python) foi descontinuado.

---

## ✨ Funcionalidades

### 🔍 Busca & Descoberta
- **Busca integrada do YouTube** - Pesquise vídeos sem abrir o navegador
- **Busca no YouTube Music** - Encontre músicas, clipes e podcasts
- **Links Diretos** - Suporte a 400+ sites via URL
- **Playlists nos resultados** - Buscas no YouTube trazem playlists junto dos vídeos; clique em **Open playlist** para abrir um modal com todos os vídeos, com botões para **Play all**, **Download all** e modo de seleção pra baixar só os marcados
- **Playlists por link** - Cole um link de playlist do YouTube (`playlist?list=` ou `watch?v=...&list=`) e a busca lista todos os vídeos

### ⬇️ Downloads Avançados
| Recurso | Descrição |
|---------|-----------|
| **Qualidade de Vídeo** | 4K (2160p), 2K (1440p), 1080p, 720p, 480p, 360p, 240p, 144p |
| **Formatos de Áudio** | MP3, M4A com extração de alta qualidade |
| **Metadados** | Incorporação automática de tags, álbum e artista |
| **Legendas** | Download e incorporação de legendas (automáticas + manuais) |
| **Retomada** | Continuar downloads interrompidos |

### 🔄 Conversor de Mídia
- Conversão de vídeo para vídeo (MKV, MP4, WebM)
- Extração e conversão de áudio
- Mesclagem de legendas
- Fila de conversão em lote
- Progresso em tempo real com ETA

### 📺 Player Integrado
- Motor de reprodução **GStreamer** (nativo, integrado ao GTK4)
- Prévia de vídeos antes do download
- Navegação de playlist (Prev / Play-Pause / **Stop** / Next)
- Janela de vídeo destacável

### 🎨 Personalização de Aparência
| Modo | Descrição |
|------|-----------|
| **Tema** | Claro / Escuro / Seguir Sistema |
| **Cores** | 10+ esquemas de cores (Padrão, Violeta, Esmeralda, Nordic, Gruvbox, Catppuccin, Dracula, Tokyo Night, Rosé Pine, Solarized, Monokai, Cyberpunk, BigTube Brand) |
| **Estilo** | Interface Glassmorphism moderna |

### 📊 Gerenciamento
- Histórico de downloads
- Histórico de conversões
- Histórico de buscas
- Opção de limpar dados automaticamente ao sair

---

## 🛠️ Tecnologias

| Tecnologia | Função |
|------------|--------|
| **Rust 2021** | Núcleo da aplicação (binário nativo) |
| **GTK4 + Libadwaita** | Interface nativa GNOME |
| **GStreamer** | Motor de reprodução |
| **yt-dlp** | Motor de download |
| **FFmpeg** | Conversão de mídia |
| **Cargo** | Build e gerenciamento de dependências |

> O projeto é um workspace Cargo com três crates: **`bigtube-core`** (lógica/engine), **`bigtube-cli`** (binário headless `bigtube`) e **`bigtube-gui`** (interface gráfica `bigtube-gui`).

---

## 🚀 Instalação

### Arch Linux (AUR) — recomendado
Pacote binário pré-compilado (`bigtube-bin`): instala rápido, **sem compilar nada** na sua máquina.
```bash
yay -S bigtube-bin
# ou
paru -S bigtube-bin
```
> O binário fornece e substitui o antigo pacote `bigtube` (`provides=bigtube`, `conflicts=bigtube`).

### Compilando do código-fonte (Cargo)
Requer o toolchain Rust (`rustup`) e as dependências de sistema listadas abaixo.
```bash
# Clone o repositório
git clone https://github.com/eltonfabricio10/python-bigtube.git
cd python-bigtube/rust

# Compile em modo release
cargo build --release --locked

# Os binários ficam em rust/target/release/
./target/release/bigtube-gui      # interface gráfica
./target/release/bigtube --help   # modo headless (CLI)
```

Para instalar no sistema a partir do build local:
```bash
sudo install -Dm755 target/release/bigtube-gui /usr/bin/bigtube-gui
sudo install -Dm755 target/release/bigtube     /usr/bin/bigtube
sudo install -Dm644 ../src/bigtube/data/bigtube.svg /usr/share/icons/hicolor/scalable/apps/bigtube.svg
sudo install -Dm644 ../src/bigtube/data/bigtube.png /usr/share/icons/hicolor/512x512/apps/bigtube.png
sudo install -Dm644 packaging/org.big.bigtube.desktop /usr/share/applications/org.big.bigtube.desktop
```

---

## ⌨️ Linha de Comando

O port Rust expõe **dois binários**:

| Binário | Função |
|---------|--------|
| `bigtube-gui` | Abre a interface gráfica |
| `bigtube` | Modo headless (download direto pelo terminal, sem GUI) |

### Interface gráfica
```bash
bigtube-gui      # abre a janela do BigTube
```

### Modo headless (`bigtube`)
```bash
bigtube -d <URL> [opções]
```

| Opção | Descrição |
|-------|-----------|
| `-d, --download URL` | Baixa a URL direto pelo terminal, sem abrir a janela |
| `-o, --output DIR` | Pasta de destino do `--download` (padrão: pasta configurada) |
| `--audio-only` | Com `--download`, extrai áudio em MP3 |
| `--format FMT` | Com `--download`, seletor de formato customizado do `yt-dlp -f` |
| `--yt-dlp-version` | Mostra a versão do `yt-dlp` embutido |
| `--version` | Mostra a versão do BigTube |
| `--help` | Mostra ajuda |

### Exemplos
```bash
bigtube-gui                                      # abre a GUI
bigtube -d https://youtube.com/watch?v=...       # download headless
bigtube -d <url> -o ~/Music --audio-only         # áudio MP3 headless
bigtube -d <url> --format "bestvideo+bestaudio"  # formato customizado
```

---

## 📁 Estrutura de Diretórios

| Localização | Conteúdo |
|-------------|----------|
| `~/.config/bigtube/` | Configurações e históricos |
| `~/.config/bigtube/config.json` | Configurações do aplicativo |
| `~/.config/bigtube/history.json` | Histórico de downloads |
| `~/.local/share/bigtube/bin/` | Binários (yt-dlp) |
| `~/.cache/bigtube/thumbnails/` | Cache de miniaturas |
| `~/Downloads/BigTube/` | Pasta padrão de downloads |

---

## ⚙️ Configurações Disponíveis

As preferências são salvas em `~/.config/bigtube/config.json`. Quando o arquivo não existe ou está corrompido, o BigTube recria a configuração com os valores padrão. Caminhos vazios ou opções desativadas simplesmente fazem o aplicativo usar o comportamento padrão.

### Aparência e componentes
| Configuração | Padrão | Explicação |
|--------------|--------|------------|
| **Tema da interface** | Seguir sistema | Define se a interface usa o tema do sistema, força tema claro ou força tema escuro. |
| **Esquema de cores** | Default Blue | Altera a paleta/acento visual da interface. Opções: Default Blue, Modern Violet, Emerald Green, Sunburst Orange, Vibrant Rose, Nordic Cyan, Nordic Snow, Gruvbox Retro, Catppuccin Mocha, Dracula Dark, Tokyo Night, Rosé Pine, Solarized Dark, Monokai Pro, Cyberpunk Neon e BigTube Brand. |
| **Versão atual / atualizar componentes** | Automático | Mostra a versão local do `yt-dlp` e permite atualizar os componentes baixados pelo app, como `yt-dlp` e `deno`, em `~/.local/share/bigtube/bin/`. |

### Busca
| Configuração | Padrão | Explicação |
|--------------|--------|------------|
| **Salvar histórico de busca** | Ativado | Guarda localmente as pesquisas feitas em `search_history.json`, permitindo reutilizar consultas anteriores. |
| **Ativar sugestões de busca** | Ativado | Mostra sugestões enquanto você digita, usando o histórico local de buscas. |
| **Máximo de sugestões** | 10 | Define quantas sugestões podem aparecer por vez. Aceita valores de 1 a 50. |
| **Limpar histórico de busca** | Ação manual | Remove todas as entradas salvas do histórico de busca. Não apaga arquivos baixados. |
| **Máximo de resultados de busca** | 15 | Define quantos resultados o BigTube pede ao `yt-dlp` em buscas por texto. Aceita valores de 5 a 100. |

### Downloads
| Configuração | Padrão | Explicação |
|--------------|--------|------------|
| **Downloads simultâneos** | 3 | Controla quantos vídeos podem baixar ao mesmo tempo. Aceita valores de 1 a 10. |
| **Pasta de download** | `~/Downloads/BigTube/` | Define onde os arquivos baixados são salvos. O app cria a pasta quando necessário. |
| **Monitor da área de transferência** | Desativado | Detecta automaticamente links de vídeo copiados para a área de transferência enquanto o app está aberto. |
| **Notificações do sistema** | Ativado | Controla avisos do sistema para eventos e erros de download. |
| **Qualidade preferida** | Perguntar sempre | Define o formato padrão para novos downloads. Pode perguntar a cada download, baixar o melhor vídeo, escolher 4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p ou baixar somente áudio em MP3/M4A. |
| **Adicionar metadados** | Desativado | Tenta incorporar artista, álbum, capa e outros metadados aos arquivos baixados. Requer `ffmpeg`; se ele não estiver instalado, o app ignora essa etapa. |
| **Incorporar legendas** | Desativado | Tenta baixar legendas manuais e automáticas e incorporá-las ao arquivo final. Atualmente procura idiomas `en.*`, `pt.*` e `es.*`. Requer `ffmpeg`. |
| **Fragmentos simultâneos** | 4 | Define quantos fragmentos paralelos o `yt-dlp` usa por download. Aceita valores de 1 a 16. Valores maiores podem acelerar downloads segmentados, mas também aumentam uso de rede. |
| **Limite de velocidade** | 0 KB/s | Limita a velocidade do download em KB/s. `0` significa sem limite. |
| **Comando de pós-processamento** | Vazio | Executa um comando após o download usando `yt-dlp --exec`. Use `{}` no comando para representar o arquivo baixado. |
| **Arquivo de cookies** | Vazio | Usa um arquivo `cookies.txt` no formato Netscape com `yt-dlp --cookies`, útil para conteúdo que exige sessão autenticada. |
| **Cookies do navegador** | Nenhum | Importa cookies diretamente de um navegador detectado, como Firefox, Chrome, Chromium, Brave, Microsoft Edge, Vivaldi ou Opera, usando `yt-dlp --cookies-from-browser`. |
| **User-Agent** | Padrão do BigTube | Sobrescreve o User-Agent enviado ao `yt-dlp`. Se ficar vazio, o app usa um User-Agent seguro baseado em Chrome. |
| **Proxy** | Vazio | Envia buscas, metadados, player e downloads pelo proxy informado. Aceita URLs `http`, `https`, `socks4`, `socks4a`, `socks5` e `socks5h`, por exemplo `socks5://127.0.0.1:1080`. |
| **Salvar histórico de downloads** | Ativado | Mantém um registro local dos downloads em `history.json`, usado pela tela de histórico/lista. |

#### Opções de qualidade
| Opção | Explicação |
|-------|------------|
| **Perguntar sempre** | Mostra a escolha de qualidade/formato no momento do download. |
| **Best (MKV)** | Baixa a melhor combinação de vídeo e áudio disponível e mescla o resultado. |
| **4K, 2K, 1080p, 720p, 480p, 360p, 240p, 144p** | Prioriza vídeo MP4/AVC na resolução escolhida com áudio M4A; se não existir exatamente esse formato, o `yt-dlp` usa a melhor alternativa compatível definida no preset. |
| **Audio (MP3)** | Extrai somente o áudio, converte para MP3 com qualidade alta e tenta incorporar miniatura. |
| **Audio (M4A)** | Baixa somente áudio priorizando codec/container M4A. |

### Conversor de mídia
| Configuração | Padrão | Explicação |
|--------------|--------|------------|
| **Salvar na pasta de origem** | Desativado | Quando ativado, o arquivo convertido é salvo ao lado do arquivo original. |
| **Pasta de saída padrão** | `~/Downloads/BigTube/Converted/` | Define a pasta usada pelo conversor quando a opção de salvar na pasta de origem está desativada. |
| **Salvar histórico de conversões** | Ativado | Mantém um registro local das conversões em `converter_history.json`. |

### Armazenamento e privacidade
| Configuração | Padrão | Explicação |
|--------------|--------|------------|
| **Limpar dados ao sair** | Desativado | Ao fechar o app, limpa os históricos de downloads, buscas e conversões. A configuração do app é preservada. Quando ativada, as opções de salvar histórico ficam desabilitadas na interface. |
| **Exportar histórico** | Ação manual | Salva o histórico de downloads em um arquivo JSON, por padrão `bigtube_history.json`. |
| **Importar histórico** | Ação manual | Restaura um histórico de downloads a partir de um arquivo JSON válido. |
| **Limpar todos os dados do app** | Ação manual | Apaga permanentemente `config.json`, `history.json`, `search_history.json` e `converter_history.json`, recria a configuração padrão e encerra o aplicativo. |

### Chaves do `config.json`
| Chave | Valor padrão | Usada por |
|-------|--------------|-----------|
| `download_path` | `~/Downloads/BigTube/` | Pasta de download |
| `theme_mode` | `system` | Tema da interface |
| `theme_color` | `default` | Esquema de cores |
| `default_quality` | `ask` | Qualidade preferida |
| `max_concurrent_downloads` | `3` | Downloads simultâneos |
| `add_metadata` | `false` | Metadados nos downloads |
| `embed_subtitles` | `false` | Legendas nos downloads |
| `save_history` | `true` | Histórico de downloads |
| `save_search_history` | `true` | Histórico de busca |
| `enable_suggestions` | `true` | Sugestões de busca |
| `max_suggestions` | `10` | Quantidade de sugestões |
| `search_limit` | `15` | Quantidade de resultados de busca |
| `save_converter_history` | `true` | Histórico do conversor |
| `auto_clear_finished` | `false` | Limpeza de históricos ao sair |
| `converter_path` | `~/Downloads/BigTube/Converted/` | Pasta de saída do conversor |
| `use_source_folder` | `false` | Conversor salvar na origem |
| `monitor_clipboard` | `false` | Monitor da área de transferência |
| `concurrent_fragments` | `4` | Fragmentos paralelos por download |
| `rate_limit` | `0` | Limite de velocidade em KB/s |
| `system_notifications` | `true` | Notificações do sistema |
| `post_process_cmd` | `""` | Comando pós-download |
| `cookies_file` | `""` | Arquivo de cookies |
| `cookies_browser` | `""` | Cookies do navegador |
| `user_agent` | `""` | User-Agent customizado |
| `proxy` | `""` | Proxy |

> Compatibilidade: configurações antigas com a chave `download_subtitles` são migradas automaticamente para `embed_subtitles`.

---

## 📋 Dependências do Sistema

Tempo de execução (necessário para rodar o binário):

```bash
# Arch Linux
sudo pacman -S gtk4 libadwaita gstreamer gst-plugins-base gst-plugins-good \
               gst-plugins-bad gst-plugin-gtk4 yt-dlp
# opcional: ffmpeg (extração de áudio e conversão de mídia)
sudo pacman -S ffmpeg

# Ubuntu/Debian (22.04+)
sudo apt install libgtk-4-1 libadwaita-1-0 \
                 gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
                 gstreamer1.0-plugins-bad gstreamer1.0-gtk4 yt-dlp ffmpeg

# Fedora
sudo dnf install gtk4 libadwaita gstreamer1-plugins-base \
                 gstreamer1-plugins-good gstreamer1-plugins-bad-free \
                 yt-dlp ffmpeg
```

Para **compilar do código-fonte** adicione o toolchain Rust e os headers de desenvolvimento:

```bash
# Arch Linux
sudo pacman -S rustup gtk4 libadwaita gstreamer base-devel
rustup default stable
```

---

## 🤝 Contribuindo

Contribuições são bem-vindas! Sinta-se à vontade para:

1. Abrir uma **Issue** para reportar bugs ou sugerir funcionalidades
2. Enviar um **Pull Request** com melhorias
3. Ajudar com traduções

---

## 📄 Licença

Este projeto está sob a licença **MIT**. Veja o arquivo [LICENSE](LICENSE) para mais detalhes.

---

<p align="center">
  Desenvolvido com ❤️ por <a href="https://github.com/eltonfabricio10">eltonff</a>
</p>
