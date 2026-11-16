# Rilis RadialDeck

Panduan untuk pemilik rilis. Hanya pemilik repository yang dapat membuat tag,
mem-publish GitHub Release, dan memperbarui AUR.

## Alur rilis

Tag `vX.Y.Z` mendorong workflow `.github/workflows/release.yml` yang membangun,
menguji, dan mem-publish GitHub Release dengan tiga aset: binary `radial-deck`
(x86_64), `radialdeck-X.Y.Z.tar.gz`, dan `radialdeck-X.Y.Z.sha256`. Setelah
rilis publik, sinkronkan AUR.

1. **Perbarui versi** di `Cargo.toml` workspace (kedua crate mewarisi
   `version.workspace = true`).
2. **Perbarui `CHANGELOG.md`**: tambah entri versi baru di atas, isi perubahan
   sejak rilis terakhir, tanggal ISO-8601.
3. **Perbarui `docs/status.md`**: snapshot tanggal dan versi; tambahkan baris
   verifikasi rilis; hapus known issue yang sudah diperbaiki.
4. **Jalankan quality gates lokal** (lihat di bawah).
5. **Commit + push ke `main`**, lalu tag:

   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z: ringkasan singkat" && git push origin vX.Y.Z
   ```

6. **Tunggu workflow Release sukses** (sekitar 7 menit).
7. **Sinkronkan AUR**:

   ```bash
   ./scripts/make-release.sh --version X.Y.Z
   ```

   Skrip mengunduh `radialdeck-X.Y.Z.tar.gz` dari GitHub Release, menghitung
   sha256-nya, menulis `packaging/aur/PKGBUILD`, dan meregenerasi
   `packaging/aur/.SRCINFO`. Kedua berkas itu di-gitignore; dorong ke remote
   git AUR, bukan ke repo upstream.

## Validasi sebelum tag

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Tambahkan uji konkret setiap perubahan perilaku, dan jelaskan hasilnya di
catatan rilis. Untuk perubahan UI, jalankan `scripts/ui-smoke.sh`; untuk
perubahan konverter, lakukan satu konversi nyata dan periksa berkas hasil.

## Publikasi

- **Jangan menebak `sha256sums`.** `scripts/make-release.sh` mengunduh
  `radialdeck-X.Y.Z.tar.gz` dari GitHub Release dan menghitung checksum
  aslinya. Verifikasi manual:

  ```bash
  curl -L "https://github.com/ReyyIchiro/radialdeck/releases/download/v$VERSION/radialdeck-$VERSION.tar.gz" | sha256sum
  ```

- Tag hanya **setelah** commit rilis lolos quality gates lokal.
- Rilis GitHub di-publish otomatis oleh workflow saat tag di-push. Sertakan
  catatan rilis lengkap via `gh release edit vX.Y.Z --notes-file FILE` bila
  perlu melebihi satu baris.
- AUR di-update setelah rilis publik; jalankan
  `./scripts/make-release.sh --version X.Y.Z`, verifikasi
  `makepkg --printsrcinfo`, lalu dorong `PKGBUILD` + `.SRCINFO` ke git AUR.

## AUR account (sekali saja)

Push ke AUR butuh akun aur.archlinux.org dan SSH key. Tanpa itu, langkah 7
di atas akan gagal dengan `Permission denied (publickey)`.

1. Buat akun di <https://aur.archlinux.org/register/> (butuh email aktif). Saat ini
   pendaftaran ditutup sementara oleh Arch; pantau
   [aur-general](https://lists.archlinux.org/listinfo/aur-general) untuk pengumuman
   pembukaan.
2. Buat SSH key khusus AUR: `ssh-keygen -t ed25519 -f ~/.ssh/aur`.
3. Tempel isi `~/.ssh/aur.pub` ke profil AUR → *My Account → SSH Public Keys*.
4. Tambahkan blok di `~/.ssh/config`:

   ```
   Host aur.archlinux.org
     User aur
     IdentityFile ~/.ssh/aur
   ```

5. Uji koneksi: `ssh aur@aur.archlinux.org help`.

## Push pertama ke AUR

Setelah akun dan SSH siap, dorong `PKGBUILD` + `.SRCINFO` dari
`packaging/aur/` ke repositori AUR `radial-deck`:

```bash
cd packaging/aur
git -c init.defaultBranch=master clone ssh://aur@aur.archlinux.org/radial-deck.git aur-remote
cp PKGBUILD .SRCINFO aur-remote/
cd aur-remote
git add PKGBUILD .SRCINFO
git commit -m "Initial release 0.3.0"
git push
```

Rilis berikutnya cukup jalankan `scripts/make-release.sh` di root repo,
salin `PKGBUILD` + `.SRCINFO` yang dihasilkan ke clone AUR, commit, dan push.

## Keamanan

Jangan menampilkan paket konverter opsional sebagai wajib. `ffmpeg`,
`imagemagick`, dll. tetap opsional; aplikasi tetap berjalan tanpa mereka.
Instalasi paket sistem dilakukan secara manual oleh pengguna melalui package
manager distribusi masing-masing, bukan melalui aplikasi.

Jika menemukan kerentanan, jangan buat isu publik. Kirim melalui
[GitHub Private Vulnerability Reporting](https://github.com/ReyyIchiro/radialdeck/security/advisories/new).
