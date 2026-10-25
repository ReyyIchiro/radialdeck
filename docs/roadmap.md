# Roadmap

Status terverifikasi ada di [`status.md`](status.md). Dokumen ini menjelaskan arah dan
keputusan platform, bukan checklist harian.

## Prinsip portabilitas

- `deck-core` tetap bebas UI/OS. Model, validasi, persistensi, pencarian, registry, dan
  eksekusi konversi dipakai ulang apa adanya.
- Shell platform hanya mengurus: jendela overlay, hotkey global, tray, autostart,
  clipboard, membuka file/folder, notifikasi, dan dialog file.
- UX dan string roda dijaga sama; perbedaan hanya pada mekanisme OS.
- Jangan mem-port GTK ke Windows/macOS. GTK di kedua OS membawa runtime besar, integrasi
  tray/aksesibilitas lemah, dan tidak menyelesaikan overlay non-activating.

## Tahap 1 — Linux (selesai)

Cakupan: satu roda tiga mode (Convert, Tools, Actions), dua hotkey portal, layer-shell,
tray, autostart, Dolphin service menu, konversi lokal, 26 file tool dengan form parameter,
Float shelf, konfigurasi schema 3, installer user-local, PKGBUILD, CI.

Sisa tahap Linux:

| Item | Alasan | Ukuran |
|---|---|---|
| Preview sebelum eksekusi tool | Tangerine menampilkan hasil sebelum menulis | besar |
| Progres + batal untuk tool panjang | Video/PDF besar memblokir thread kerja | sedang |
| Flatpak | Distribusi aman; konverter perlu akses host | sedang |
| Aksen warna KDE | Panel lebih menyatu dengan Plasma | kecil |
| Audit aksesibilitas | Keyboard sudah baik; screen reader belum diuji | kecil |
| Posisi kursor X11 | Keterbatasan GTK4; overlay mengisi monitor pointer | kecil |

## Tahap 2 — Windows

**Keputusan: shell Tauri v2 + WebView2, memakai `deck-core` sebagai Rust backend.**

Alasan:

- WebView2 sudah tersedia di Windows 10/11; tidak membundel Chromium.
- Tauri menyediakan tray, global shortcut, single-instance, autostart, notifikasi, dan
  updater yang matang.
- Roda dapat digambar dengan Canvas 2D; animasi 60 fps lebih dari cukup.
- Ukuran installer wajar (±5–10 MB tanpa runtime), RAM idle jauh lebih rendah daripada
  Electron.
- Backend Rust memanggil `deck-core` langsung; tidak ada duplikasi logika.

Kebutuhan shell:

1. Jendela borderless, topmost, `skipTaskbar`, transparan, fullscreen satu monitor.
2. Dua hotkey: `Ctrl+Alt+Space` dan `Ctrl+Alt+Q`, dapat diubah.
3. Tray dengan menu yang sama; close ke tray.
4. Autostart lewat registry `Run` atau Startup folder; sediakan toggle.
5. Buka folder di Explorer, terminal (Windows Terminal/PowerShell), salin path.
6. Konverter: bundle `magick`/`ffmpeg` atau deteksi instalasi; Office lewat COM hanya jika
   eksplisit diaktifkan.
7. Pengaturan DPI per-monitor; posisi panel di kursor monitor aktif.
8. Installer NSIS/MSI, signature code (opsional tahap awal).

Kriteria selesai: dua roda berfungsi, data config kompatibel (path Windows), konversi
gambar/audio/video dasar, tray/hotkey/autostart, tanpa crash saat monitor dicabut.

## Tahap 3 — macOS

**Keputusan: shell SwiftUI/AppKit native, memakai `deck-core` lewat FFI crate (`cdylib` +
header) atau sidecar Rust.**

Alasan:

- UX referensi (ProjectDeck) adalah AppKit; `NSPanel` non-activating, `LSUIElement`, dan
  Carbon hotkey memberi perilaku yang benar di Spaces/fullscreen.
- Sandbox App Store membatasi path dan konverter; distribusi di luar App Store lebih
  realistis untuk tahap awal.
- WebView Tauri di macOS menambah lapisan yang tidak perlu untuk panel kecil; native
  memberi startup dan memori terbaik.

Kebutuhan shell:

1. `LSUIElement=true`, tray `NSStatusItem`, tanpa Dock icon.
2. Dua hotkey via Carbon `RegisterEventHotKey` (tanpa izin Accessibility).
3. `NSPanel` non-activating, level `.floating`/`.statusBar`, collect beats; posisi di
   kursor layar aktif; clamp tepi.
4. Folder access: native path tanpa sandbox; untuk App Store gunakan security-scoped
   bookmarks (seperti referensi).
5. Finder Services/Quick Actions untuk “Open with RadialDeck”.
6. Konversi via `sips`, AVFoundation, dan tool bundled; PDF/Office via LibreOffice bila
   tersedia.
7. Notarization + hardened runtime; `SMAppService` untuk launch at login.

Kriteria selesai: panel muncul di Space aktif tanpa mengubah aplikasi fokus, dua hotkey,
Finder service, tray, launch at login, konversi inti, config di Application Support.

## Tahap 4 — Distribusi dan kualitas

- Flatpak (Linux), MSIX/NSIS (Windows), DMG (macOS).
- Update channel tanpa telemetry; cek rilis manual dari menu.
- Audit dependensi (`cargo audit`, `cargo deny`) di CI.
- Benchmark startup/idle per platform; target <100 MB RSS saat panel tertutup.
- Dokumentasi pengguna dengan tangkapan layar tiap platform.

## Yang sengaja tidak dilakukan

- Electron: berat untuk utilitas tray.
- Menyatukan shell Linux ke Tauri: layer-shell dan fokus Wayland lebih baik dengan GTK4
  native; Tauri di Linux belum memberi overlay layer-shell yang setara.
- Menyalin kode ProjectDeck/Tangerine: lisensi dan arsitektur berbeda; hanya UX yang
  dijadikan acuan.
