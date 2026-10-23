# Berkontribusi ke RadialDeck

Terima kasih atas minatmu! Dokumen ini menjelaskan standar dan proses kontribusi.

## Prasyarat

- Rust stable terbaru (`rustup update stable`)
- gtk4-layer-shell (lihat README untuk build deps)
- `cargo clippy` dan `cargo fmt` (komponen rustup, terinstall otomatis)

## Setup

```bash
git clone https://github.com/raihan/radial-deck.git
cd radial-deck
cargo build
```

## Standar Kode

- **Format:** `cargo fmt --all` sebelum commit; CI akan gagal jika tidak.
- **Lint:** `cargo clippy --all-targets -- -D warnings`; tidak boleh ada warning.
- **Test:** `cargo test --all`; semua harus lulus.
- **Komentar & identifier:** bahasa Inggris.
- **Dokumentasi & komunikasi:** bahasa Indonesia boleh.

## Arsitektur Platform

Kode platform-spesifik **hanya** boleh ada di `crates/radial-deck/src/` (Linux).
Logika bisnis (`deck-core`) tidak boleh mengimport GTK, gio, atau platform crate apapun.

Untuk port baru, buat crate baru (mis. `crates/radial-deck-windows/`) yang memakai
`deck-core` dan mengimplementasikan lapisan platform yang setara.

## Konvensi Commit

Gunakan [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(wheel): tambah navigasi mouse back/forward
fix(hotkey): perbaiki panic saat portal tidak tersedia
refactor(core): pisahkan search scoring ke modul tersendiri
docs: perbarui README cara install user-local
chore: bump gtk4 ke 0.12
```

Type yang valid: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

Breaking change: tambahkan `!` setelah type, mis. `feat!: ubah format config`.

## Pull Request

1. Fork → buat branch `feat/nama-fitur` atau `fix/isu-singkat`.
2. Pastikan `cargo fmt && cargo clippy && cargo test` semua bersih.
3. Deskripsi PR: apa yang diubah, mengapa, cara menguji.
4. Satu PR = satu perubahan logis.

## Melaporkan Bug

Buka GitHub Issue dengan:
- Versi (`radial-deck --version`)
- OS, desktop, sesi (Wayland/X11)
- Langkah reproduksi
- Output log (`RUST_LOG=radial_deck=debug radial-deck 2>&1`)

## Keamanan

Jangan buka issue publik untuk kerentanan keamanan.
Kirim ke email maintainer langsung.
