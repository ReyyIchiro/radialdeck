# Menginstal RadialDeck

Panduan pemasangan untuk pengguna. Pengembang: lihat `CONTRIBUTING.md` dan `docs/releasing.md`.

## Build dari sumber (cara utama saat ini)

```bash
sudo pacman -S gtk4 gtk4-layer-shell xdg-desktop-portal rust  # Arch/CachyOS

git clone https://github.com/ReyyIchiro/radialdeck.git
cd radialdeck
./scripts/install.sh
```

Skrip memasang binary release ke `~/.local/bin/radial-deck`, bersama desktop entry, ikon, dan service menu Dolphin. Jalankan `radial-deck --hidden` sekali, lalu setujui *Global Shortcuts* di KDE Portal agar hotkey aktif. Aktifkan *Launch at login* di Settings supaya hotkey tetap hidup setelah reboot.

Untuk distribusi non-Arch, ganti langkah `pacman` dengan padanannya (`apt install libgtk-4-dev libgtk-4-layer-shell-dev xdg-desktop-portal rust` di Debian/Ubuntu, `dnf install ...` di Fedora).

## AUR (coming soon)

Paket AUR `radial-deck` sedang dipersiapkan. Pendaftaran akun AUR ditutup sementara oleh Arch ([aur.archlinux.org](https://aur.archlinux.org/), "New account registration is temporarily closed"), jadi paket belum bisa di-publish. Begitu akun maintainer aktif, `paru -S radial-deck` dan `yay -S radial-deck` akan tersedia.

Pemilik proyek yang ingin menyiapkan publish: lihat `docs/releasing.md`.

## Flatpak (WIP)

Manifest ada di `packaging/flatpak/`, tapi sandbox belum bisa memakai binary host seperti `ffmpeg`. Belum direkomendasikan untuk pengguna biasa.

## Konverter opsional

Fitur roda menyesuaikan diri dengan alat yang terpasang di sistem:

| Kategori | Paket opsi | Efek jika tidak terpasang |
|---|---|---|
| Gambar | `imagemagick` | Beberapa output gambar ditandai tidak tersedia |
| Video/Audio | `ffmpeg` | Konverter video dan audio tidak tersedia |
| PDF | `poppler`, `ghostscript` | Konversi dan kompresi PDF terbatas |
| Dokumen | `libreoffice-fresh`, `pandoc` | Konversi dokumen dan slide terbatas |
| Arsip | `7zip` | Ekstraksi arsip non-native (7z/rar) tidak tersedia |
| Kirim | `kdeconnect` | Aksi *Send* ditandai tidak tersedia |

Instal paket yang dibutuhkan lewat package manager sistem, lalu tekan **Recheck** di *Settings → Dependencies*. RadialDeck tidak pernah menjalankan package manager atau `sudo` sendiri.

Semua konverter dipanggil sebagai proses terpisah tanpa shell; nama berkas dan parameter pengguna tidak pernah masuk ke baris perintah mentah. Semua output ditulis dengan nama unik sehingga berkas lama tidak pernah ditimpa.

## Perbarui dan hapus

- Sumber: tarik ulang, build ulang, dan jalankan `./scripts/install.sh` lagi.
- Hapus: jalankan `./scripts/uninstall.sh` dari direktori sumber yang dipakai saat pemasangan.
- AUR (begitu tersedia): `yay -Syu radial-deck` atau `paru -Syu radial-deck`.

## Hotkey tidak aktif?

Aplikasi harus berjalan untuk memegang hotkey global. Aktifkan *Launch at login* di pengaturan aplikasi, atau pastikan berkas `~/.config/autostart/io.github.raihan.RadialDeck.desktop` ada. Jika masih tidak aktif, jalankan `./scripts/doctor.sh` atau periksa *Settings → Shortcuts → Change Shortcuts...* di KDE.

## Menjadi maintainer AUR (sekali saja)

Panduan untuk pemilik proyek. Hanya diperlukan saat pertama kali memublikasikan paket atau memindah kepemilikan.

1. Buat akun di [aur.archlinux.org/register](https://aur.archlinux.org/register/) (butuh email aktif untuk verifikasi). Saat ini pendaftaran ditutup sementara; pantau [aur-general](https://lists.archlinux.org/listinfo/aur-general) untuk pengumuman pembukaan.
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

Setelah akun aktif, buat repositori paket lokal dan dorong `PKGBUILD` + `.SRCINFO` dari `packaging/aur/`. Rinciannya ada di `docs/releasing.md`.

## Hak cipta

RadialDeck dilisensikan MIT. Lihat `LICENSE`.
