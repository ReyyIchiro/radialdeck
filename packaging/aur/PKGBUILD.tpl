# Maintainer: RadialDeck contributors
#
# TEMPLATE — do not build from this file directly.
#
# This template is rendered by `scripts/make-release.sh` into a final
# PKGBUILD (plus SRCINFO) once a release source artifact exists, so the
# sha256 below is always computed from the real release tarball.
# An AUR submission is only valid after:
#   1. the `v{{PKGVER}}` tag is pushed and the release workflow has
#      published the source artifact to the GitHub release, and
#   2. `scripts/make-release.sh --version {{PKGVER}}` has rendered this
#      template with the real artifact checksum.
#
# Placeholders: {{PKGVER}}, {{SHA256}}

pkgname=radial-deck
pkgver={{PKGVER}}
pkgrel=1
pkgdesc="Radial file conversion and project folder launcher (Wayland, KDE Plasma)"
arch=('x86_64' 'aarch64')
url="https://github.com/ReyyIchiro/radialdeck"
license=('MIT')
depends=(
    'gtk4'
    'gtk4-layer-shell'
    'xdg-desktop-portal'
    'xdg-utils'
    'hicolor-icon-theme'
)
optdepends=(
    'imagemagick: HEIC/AVIF/PDF image output and extra input formats'
    'ffmpeg: video and audio conversions, trim/split/snapshot tools'
    'poppler: PDF to image/text conversions and page-level PDF tools'
    'ghostscript: PDF compression'
    'libreoffice-fresh: document, slide and spreadsheet conversions (libreoffice-still also works)'
    'pandoc: document and EPUB to Markdown conversions'
    '7zip: 7Z and RAR extraction'
    'kdeconnect: Send action over the local network'
)
makedepends=('cargo' 'rust')
# Deterministic source artifact produced by the release workflow
# (git archive of the tag, root directory $pkgname-v$pkgver).
source=("$url/releases/download/v$pkgver/$pkgname-v$pkgver-source.tar.gz")
sha256sums=({{SHA256}})

prepare() {
    cd "$pkgname-$pkgver"
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --frozen
}

package() {
    cd "$pkgname-$pkgver"

    install -Dm755 "target/release/radial-deck" \
        "$pkgdir/usr/bin/radial-deck"

    install -Dm644 "packaging/io.github.raihan.RadialDeck.desktop" \
        "$pkgdir/usr/share/applications/io.github.raihan.RadialDeck.desktop"

    install -Dm644 "packaging/io.github.raihan.RadialDeck.svg" \
        "$pkgdir/usr/share/icons/hicolor/scalable/apps/io.github.raihan.RadialDeck.svg"

    install -Dm644 "packaging/io.github.raihan.RadialDeck.servicemenu.desktop" \
        "$pkgdir/usr/share/kio/servicemenus/io.github.raihan.RadialDeck.desktop"

    install -Dm644 LICENSE \
        "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
