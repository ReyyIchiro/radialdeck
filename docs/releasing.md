# Rilis RadialDeck

Dokumen ini adalah panduan untuk pemilik rilis. Hanya eksekusi rilis oleh pemilik repository yang dapat membuat tag, publikasi GitHub Release, dan memperbarui AUR. Seluruh file di bawah ini harus diperbarui sebelum melakukan tag.

## Alur rilis

1. **Perbarui versi** di `crates/deck-core/Cargo.toml`, `crates/radial-deck/Cargo.toml`, dan `Cargo.toml` workspace.
2. **Perbarui `CHANGELOG.md`**: tambah entri versi baru di atas, isi perubahan yang efektif ditambahkan sejak rilis terakhir, dan tanggal ISO-8601.
3. **Perbarui `docs/status.md`**: Snapshot tanggal dan versi; tambahkan atau perbarui baris verifikasi rilis; hapus known issue yang sudah diperbaiki.
4. **Perbarui `packaging/aur/PKGBUILD`**: `pkgver`, `sha256sums` (dari arsip tag asli), dan versi terkait. Jangan pernah menebak checksum.
5. **Buat tag** `v<versi>` dari commit rilis yang diuji, lalu publikasikan ke GitHub.
6. **Buat GitHub Release** dari tag: sertakan catatan rilis, arsip sumber otomatis GitHub, dan artefak build yang diuji dengan checksum SHA-256.
7. **Perbarui AUR** setelah rilis publikasi; verifikasi `makepkg --printsrcinfo` dan build paket secara lokal.

## Validasi sebelum tag

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Tambahkan uji konkret setiap perubahan perilaku, dan jelaskan hasilnya di catatan rilis. Untuk perubahan UI, jalankan `scripts/ui-smoke.sh`; untuk perubahan konverter, lakukan satu konversi nyata dan periksa berkas hasil.

## Publikasi

- Jangan menebak `sha256sums`. Hasilkan checksum dari arsip tag sebenarnya:
  ```bash
  curl -L "https://github.com/ReyyIchiro/radialdeck/archive/v$VERSION/radialdeck-$VERSION.tar.gz" | sha256sum
  ```
- Buat tag **setelah** commit rilis diuji, bukan sebelum.
- Publikasikan rilis hanya ketika artefak build dan checksum siap. Sertakan catatan rilis yang mencantumkan fitur baru, perbaikan bug, dan peringatan migrasi (jika ada).
- Setiap rilis besar harus menyertakan catatan "Bersama" yang menjelaskan apa yang berubah dan apa yang harus diperhatikan pengguna.

## Keamanan

Jangan menampilkan paket konverter opsional sebagai wajib. `ffmpeg`, `imagemagick`, dll. tetap opsional; aplikasi tetap berjalan tanpa mereka. Instalasi paket sistem dilakukan secara manual oleh pengguna melalui package manager distribusi masing-masing, bukan melalui aplikasi.

Jika menemukan kerentanan, jangan buat isu publik. Kirim melalui [GitHub Private Vulnerability Reporting](https://github.com/ReyyIchiro/radialdeck/security/advisories/new).
