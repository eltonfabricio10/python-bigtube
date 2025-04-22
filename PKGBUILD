# Maintainer: eltonff <eltonfabricio10@gmail.com>

_pkgname=bigtube
pkgname="python-${_pkgname}"
pkgver=1.0.0
pkgrel=1
pkgdesc="Gerenciador de downloads de vídeos com interface GTK4 e Adwaita"
arch=('any')
url="https://github.com/eltonfabricio10/python-bigtube"
license=('MIT')

depends=(
    'python'
    'python-gobject'
    'gtk4'
    'libadwaita'
    'yt-dlp'
    'python-requests'
    'gst-plugin-gtk'
)

makedepends=(
    'python-build'
    'python-installer'
    'python-wheel'
    'python-setuptools'
    'git'
)

optdepends=(
    'ffmpeg'
)

source=(
    "git+${url}.git"
)

sha256sums=(
    'SKIP'
)

prepare() {
    cd "${srcdir}/${pkgname}"
    rm -rvf build dist *.egg-info
}

# Construção do pacote
build() {
    cd "${srcdir}/${pkgname}"
    python -m build --wheel --no-isolation
}

# Instalação do pacote
package() {
    cd "${srcdir}/${pkgname}"
    python -m installer --destdir="${pkgdir}" dist/*.whl
    #install -Dm644 "assets/icon.png" "${pkgdir}/usr/share/pixmaps/${_pkgname}.png"
    install -Dm644 "assets/${_pkgname}.desktop" "${pkgdir}/usr/share/applications/${_pkgname}.desktop"
    install -Dm644 README.md "${pkgdir}/usr/share/doc/${pkgname}/README.md"
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}

post_install() {
    gtk-update-icon-cache -qtf usr/share/icons/hicolor
    update-desktop-database -q
}

post_upgrade() {
    post_install
}

post_remove() {
    post_install
}
