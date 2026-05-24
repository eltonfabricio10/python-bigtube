<p align="center">
  <img src="https://raw.githubusercontent.com/eltonfabricio10/python-bigtube/main/assets/banner.png" alt="BigTube Banner" width="100%">
</p>

# 🎬 BigTube

> **O Downloader Multimídia Definitivo para Linux**

**BigTube** é uma aplicação desktop moderna, veloz e elegante, construída com **Python**, **GTK4** e **Libadwaita**. Projetado para quem não aceita menos que a perfeição ao baixar conteúdos da internet, o BigTube transforma a complexidade do `yt-dlp` em uma ferramenta intuitiva e poderosa.

---

## ✨ Funcionalidades

### 🔍 Busca & Descoberta
- **Busca integrada do YouTube** - Pesquise vídeos sem abrir o navegador
- **Busca no YouTube Music** - Encontre músicas, clipes e podcasts
- **Links Diretos** - Suporte a 400+ sites via URL
- **Playlists** - Cole um link de playlist do YouTube (`playlist?list=` ou `watch?v=...&list=`) e a busca lista todos os vídeos

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
- Motor de reprodução **MPV**
- Prévia de vídeos antes do download
- Navegação de playlist
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
| **Python 3.10+** | Núcleo da aplicação |
| **GTK4 + Libadwaita** | Interface nativa GNOME |
| **yt-dlp** | Motor de download |
| **MPV** | Motor de reprodução |
| **FFmpeg** | Conversão de mídia |
| **Poetry** | Gerenciamento de dependências |

---

## 🚀 Instalação

### Arch Linux (AUR)
```bash
yay -S bigtube
# ou
paru -S bigtube
```

### PKGBUILD Local
```bash
git clone https://github.com/eltonfabricio10/python-bigtube.git
cd python-bigtube
makepkg -si
```

### Instalação via Poetry (Desenvolvimento)
```bash
# Clone o repositório
git clone https://github.com/eltonfabricio10/python-bigtube.git
cd python-bigtube

# Instale as dependências
poetry install

# Execute o BigTube
poetry run bigtube
```

---

## ⌨️ Linha de Comando

```bash
bigtube [opções]
```

| Opção | Descrição |
|-------|-----------|
| `--debug` | Ativa log detalhado para depuração |
| `--version` | Mostra a versão do yt-dlp |
| `--help` | Mostra ajuda |
| URLs posicionais | Após `--`, URLs são abertas na busca (ex.: `bigtube -- https://youtube.com/playlist?list=...`) |

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

### Downloads
- Pasta de download personalizada
- Qualidade preferida (Perguntar / Melhor MKV / 4K-144p / Áudio)
- Adicionar metadados aos arquivos
- Incorporar legendas automaticamente

### Armazenamento
- Salvar histórico de downloads
- Salvar histórico de conversões
- Limpar todos os dados ao sair

### Conversor
- Pasta de saída padrão
- Usar mesma pasta do arquivo fonte

---

## 📋 Dependências do Sistema

```bash
# Arch Linux
sudo pacman -S gtk4 libadwaita mpv ffmpeg python-gobject

# Ubuntu/Debian (22.04+)
sudo apt install libgtk-4-1 libadwaita-1-0 mpv ffmpeg python3-gi

# Fedora
sudo dnf install gtk4 libadwaita mpv ffmpeg python3-gobject
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
