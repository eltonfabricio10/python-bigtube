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
    'python-mpv'
    'gst-plugins-base'
    'gst-plugins-good'
    'gst-plugins-bad'
    'gst-plugins-ugly'
    'gst-libav'
)

makedepends=(
    'python-build'
    'python-installer'
    'python-wheel'
    'python-setuptools'
    'git'
    'gettext'
)

optdepends=('ffmpeg')
source=("git+${url}.git")
sha256sums=('SKIP')

prepare() {
    cd "${srcdir}/${pkgname}"
    rm -rvf build dist *.egg-info
}

build() {
    cd "${srcdir}/${pkgname}"
    python -m build --wheel --no-isolation

    # Compilar traduções (.po -> .mo)
    for po in locales/*.po; do
        msgfmt "$po" -o "${po%.po}.mo"
    done
}

package() {
    cd "${srcdir}/${pkgname}"
    python -m installer --destdir="${pkgdir}" dist/*.whl
    install -Dm644 "src/bigtube/data/bigtube.png" "${pkgdir}/usr/share/icons/hicolor/256x256/apps/bigtube.png"
    install -Dm644 "src/bigtube/data/org.big.bigtube.desktop" "${pkgdir}/usr/share/applications/org.big.bigtube.desktop"
    install -Dm644 README.md "${pkgdir}/usr/share/doc/${pkgname}/README.md"
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"

    # Instalar traduções
    for mo in locales/*.mo; do
        _lang=$(basename "$mo" .mo)
        install -Dm644 "$mo" "${pkgdir}/usr/share/locale/${_lang}/LC_MESSAGES/bigtube.mo"
    done
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
