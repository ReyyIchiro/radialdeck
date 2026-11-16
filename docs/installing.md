# Menginstal RadialDeck

Dokumen ini adalah panduan pemasangan untuk pengguna. Pengembang dapat melihat `CONTRIBUTING.md` dan `docs/releasing.md`.

## Metode yang direkomendasikan: AUR (Arch/CachyOS)

RadialDeck tersedia melalui AUR. Paket ini mencakup dependensi wajib aplikasi, desktop entry, ikon, dan *service menu* Dolphin.

```bash
# Contoh dengan yay
yay -S radial-deck
# Atau dengan paru
paru -S radial-deck
```

Instalasi paket wajib menampilkan `gtk4`, `gtk4-layer-shell`, `xdg-desktop-portal`, dan `xdg-utils`.

Konverter eksternal opsional (FFmpeg, ImageMagick, Poppler, Ghostscript, LibreOffice, Pandoc, 7-Zip, KDE Connect) tidak dipasang secara otomatis. Pemasang akan menandai fitur terkait tidak tersedia; instal paket yang dibutuhkan lalu tekan **Recheck** di pengaturan aplikasi.

## Build dari sumber

Jika AUR tidak tersedia atau Anda ingin build sendiri:

```bash
git clone https://github.com/ReyyIchiro/radialdeck.git
cd radialdeck
./scripts/install.sh
```

Pemasang ini membuat binary release dan meletakkannya di `~/.local/bin/radial-deck`, junto dengan desktop entry dan ikon. Jalankan `radial-deck --hidden` sekali, lalu setujui *Global Shortcuts* di KDE Portal agar hotkey aktif.

## Konverter opsional

Fitur radial menyesuaikan diri dengan alat yang terpasang di sistem:

| Kategori | Paket opsi | Efek jika tidak terpasang |
|---|---|---|
| Gambar | `imagemagick` | Beberapa output gambar ditandai tidak tersedia |
| Video/Audio | `ffmpeg` | Konverter video dan audio tidak tersedia |
| PDF | `poppler`, `ghostscript` | Konversi PDF dan kompressi PDF terbatas |
| Dokumen | `libreoffice-fresh`, `pandoc` | Konversi dokumen dan slides terbatas |
| Arsip | `7zip` | Ekstraksi arsip non-native tidak tersedia |
| Kirim | `kdeconnect` | Aksi *Send* ditandai tidak tersedia |

Semua konverter dipanggil sebagai proses terpisah tanpa shell; nama berkas dan parameter pengguna tidak pernah masuk ke baris perintah mentah. Semua output ditulis dengan nama unik sehingga berkas lama tidak pernah ditimpa.

## Perbarui dan hapas

- AUR: perbarui paket seperti biasa (`yay -Syu radial-deck` atau `paru -Syu radial-deck`).
- Sumber: tarik ulang, build ulang, dan jalankan `./scripts/install.sh` lagi.
- Hapas: jalankan `./scripts/uninstall.sh` dari direktori sumber yang digunakan saat pemasangan.

## Hotkey tidak aktif?

Aplikasi harus berjalan untuk memegang hotkey global. Aktifkan *Launch at login* di pengaturan aplikasi, atau pastikan berkas *autostart* ada. Jika masih tidak aktif, jalankan `./scripts/doctor.sh` atau periksa *Settings → Shortcuts → Change Shortcuts…* di KDE.


## Menjadi maintainer AUR (sekali saja)

Panduan untuk pemilik proyek. Hanya diperlukan saat pertama kali
memublikasikan paket atau memindah kepemilikan.

1. Buat akun di [aur.archlinux.org/register](https://aur.archlinux.org/register/)
   (butuh email aktif untuk verifikasi).
2. Buat SSH key khusus AUR:
   ```bash
   ssh-keygen -t ed25519 -f ~/.ssh/aur -C "aur-$(whoami)"
   ```
3. Tempel isi `~/.ssh/aur.pub` ke profil AUR → *My Account → SSH Public Keys*.
4. Tambahkan blok di `~/.ssh/config`:
   ```
   Host aur.archlinux.org
     User aur
     IdentityFile ~/.ssh/aur
   ```
5. Uji koneksi: `ssh aur@aur.archlinux.org help`.

Setelah akun aktif, buat repositori paket lokal dan dorong `PKGBUILD` +
`.SRCINFO` dari `packaging/aur/`. Rinciannya ada di `docs/releasing.md`.

## Hak cipta

RadialDeck dilisensikan MIT. Lihat `LICENSE`.
