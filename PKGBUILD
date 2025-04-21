# Maintainer: eltonff <eltonfabricio10@gmail.com>

_pkgname=bigtube
pkgname="python-${_pkgname}"
pkgver=1.0.0
pkgrel=1
pkgdesc="Gerenciador de downloads de vídeos com interface GTK4 e Adwaita"
arch=('any')
url="https://github.com/eltonfabricio10/python-bigtube"
license=('MIT')

# Dependências de runtime
depends=(
    'python'
    'python-gobject'
    'gtk4'
    'libadwaita'
    'yt-dlp'
    'python-requests'
    'gst-plugin-gtk'
)

# Dependências de build
makedepends=(
    'python-build'
    'python-installer'
    'python-wheel'
    'python-setuptools'
    'git'
)

# Dependências opcionais
optdepends=(
    'ffmpeg'
)

# Fonte do pacote
source=(
    "git+${url}.git"
)

# Verificação de integridade (SHA256)
sha256sums=(
    'SKIP'  # Substitua pelo hash correto após gerar o tarball
)

# Preparação do pacote
prepare() {
    cd "${srcdir}/${pkgname}/${_pkgname}"

    # Limpar builds anteriores
    rm -rvf build dist *.egg-info
}

# Construção do pacote
build() {
    cd "${srcdir}/${pkgname}/${_pkgname}"

    # Construir pacote Python
    python -m build --wheel --no-isolation
}

# Instalação do pacote
package() {
    cd "${srcdir}/${pkgname}/${_pkgname}"

    # Instalar pacote Python
    python -m installer --destdir="${pkgdir}" dist/*.whl

    # Instalar ícone e .desktop
    #install -Dm644 "assets/icon.png" "${pkgdir}/usr/share/pixmaps/${_pkgname}.png"
    install -Dm644 "assets/${_pkgname}.desktop" "${pkgdir}/usr/share/applications/${_pkgname}.desktop"

    # Instalar documentação
    install -Dm644 README.md "${pkgdir}/usr/share/doc/${pkgname}/README.md"

    # Instalar licença
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}

# Hooks pós-instalação
post_install() {
    # Atualizar cache de ícones e aplicativos
    gtk-update-icon-cache -qtf usr/share/icons/hicolor
    update-desktop-database -q
}

post_upgrade() {
    post_install
}

post_remove() {
    post_install
}
