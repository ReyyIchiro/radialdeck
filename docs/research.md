# Riset & Keputusan Teknis

Tanggal: 2026-10-22

## 1. Referensi yang dibaca

### Tangerine (tangerineformac.com)

Aplikasi macOS berbayar (Mac App Store). Inti UX:

- Pengguna men-drag file, lalu menekan tombol modifier; muncul **radial wheel** di sekitar kursor yang tetap terbuka sampai file di-drop.
- `Shift` = konversi format, `Command` = file tools (compress, crop, trim, split, merge, dll.), `Control` = aksi file (AirDrop, Float, Copy, Move, Copy Path, Terminal).
- Semua pemrosesan lokal (tidak upload); dukungan gambar, video, audio, dokumen, spreadsheet, arsip.
- Navigasi wheel dengan panah/angka, `Enter` apply, `Escape` batal.
- Ini adalah **produk closed-source**; tidak ada kode yang dapat dipakai.

### nizar268/Shortcut-Floating (ProjectDeck)

Aplikasi macOS SwiftUI, 100% native, berjalan sebagai menu bar agent (`LSUIElement`). Fitur yang tercantum di README dan kode:

- Global hotkey `Control+Space` via Carbon `RegisterEventHotKey`.
- Panel radial melayang (`NSPanel` non-activating) yang muncul di posisi kursor, dengan clamping multi-monitor.
- Manajemen folder via drag & drop, multi-project, pencarian instan, slot angka `1-9`, navigasi panah/`Enter`/`Esc`.
- Bookmark security-scoped untuk akses folder yang persisten; pengecekan folder tidak tersedia.
- Menu bar (StatusItem), Settings, Launch at Login (`SMAppService`), tema light/dark.
- Arsitektur: `Models` / `Managers` / `FloatingLauncher` / `Management` / `Settings` / `Utilities`; persistensi JSON di Application Support.

**Lisensi repo referensi: tidak ada file LICENSE dan metadata GitHub `license: null`.** Artinya hak cipta penuh penulis; menyalin kode tidak diizinkan. Kita hanya memakai ide fitur/UX (ide tidak berhak cipta) dan menulis implementasi clean-room.

## 2. Analisis gap macOS → Linux (KDE Plasma 6 Wayland)

| Kebutuhan | macOS (ProjectDeck) | Linux Wayland (KDE 6) | Padanan yang dipakai |
|---|---|---|---|
| Global hotkey | Carbon `RegisterEventHotKey` | Tidak ada API universal; compositor membatasi | **XDG Desktop Portal `GlobalShortcuts`** (didukung portal KDE; dimediasi user) |
| Panel melayang di atas semua window | `NSPanel` + level | `xdg_toplevel` tidak bisa always-on-top / posisi absolut | **wlr-layer-shell** (`gtk4-layer-shell`), layer `overlay`, keyboard exclusive |
| Posisi panel di kursor | `NSEvent.mouseLocation` | Wayland sengaja tidak mengekspos posisi pointer global | X11: `XQueryPointer`; Wayland: anchor center pada output aktif (batasan protokol, didokumentasikan) |
| Dismiss saat klik luar | `NSEvent` global monitor | Tidak ada global monitor | Backdrop fullscreen transparan di layer yang sama; klik di luar wheel = tutup |
| Tray | `NSStatusItem` | StatusNotifierItem via D-Bus | crate `ksni` |
| Launch at login | `SMAppService` | XDG autostart | `~/.config/autostart/*.desktop` |
| Buka folder | `NSWorkspace.open` | `xdg-open` / portal OpenURI | `xdg-open` (argv tetap, tanpa shell) |
| Clipboard | `NSPasteboard` | `wl_data_device` / GTK clipboard | `gdk::Clipboard` |
| Notifikasi | `UNUserNotificationCenter` | `org.freedesktop.Notifications` | `notify-rust` |
| Bookmark folder | Security-scoped bookmark | Tidak ada sandbox untuk app native | Path absolut + validasi + status ketersediaan |
| Tema | `NSAppearance` | Portal `color-scheme` | GTK4 otomatis mengikuti sistem |
| Multi-monitor | `NSScreen` | Output dipilih compositor | Layer surface tanpa monitor eksplisit = output aktif |

## 3. Perbandingan stack

| Opsi | Ukuran/RAM | Wayland + layer-shell | Port Windows/macOS | Catatan |
|---|---|---|---|---|
| **Rust + GTK4 + gtk4-layer-shell** | Binari ~3–6 MB; RSS ~55–75 MB | Native, layer-shell resmi | Core Rust 100% portabel; UI perlu backend baru | Dipilih |
| Tauri v2 (Rust + WebKitGTK) | Bundle ~10 MB; RSS ~100–180 MB | WebView = toplevel biasa, overlay sulit | Paling mudah | Terlalu berat untuk idle utility |
| Qt 6 / C++ + LayerShellQt | Binari ~5 MB; RSS ~50 MB | Native (LayerShellQt tersedia) | Baik, tetapi Rust core perlu bridge C ABI | Kompleksitas FFI, waktu |
| Electron | ~150 MB | Overlay terbatas | Mudah | Ditolak: berat |

**Keputusan:** Rust untuk seluruh core; GTK4 untuk shell Linux; `gtk4-layer-shell` untuk overlay; portal untuk hotkey. Lapisan platform diisolasi di crate app Linux, sehingga port Windows/macOS hanya mengganti UI + integrasi (core, model, storage, search tetap sama).

## 4. Keputusan repository

**Repo baru** (bukan fork), alasan:

1. Upstream tidak punya lisensi → fork tetap tidak memberi hak memakai/mendistribusikan ulang kode.
2. Kode upstream 100% Swift/AppKit/Carbon; tidak ada bagian yang dapat dikompilasi di Linux, sehingga "fork" hanya menyisakan nama dan issue tracker.
3. Riwayat git baru membuat atribusi ide jelas di README dan menghindari klaim kompatibilitas yang menyesatkan.

Repo: `radial-deck` (binary `radial-deck`). Lisensi proyek: MIT. Atribusi ide ke ProjectDeck dan Tangerine di README.

## 5. Asumsi

- Hotkey default `Ctrl+Alt+Space` (menghindari konflik input-switch KDE `Ctrl+Space`). Portal dapat meminta konfirmasi satu kali.
- Wayland tidak memungkinkan posisi kursor global; panel muncul di tengah output aktif. Di X11 posisi mengikuti kursor.
- Manajemen file/folder memakai path asli tanpa sandbox (app native, bukan Flatpak sandbox) — Flatpak nanti memakai portal FileChooser + Document portal.
- Konversi format ala roda `Shift` Tangerine masuk MVP Linux; file tools lain (crop, trim,
  merge, AirDrop/Float) belum. Prioritas: format conversion yang deterministik dan dapat
  diverifikasi dengan tool CLI yang sudah lazim di Linux.

## 6. Implementasi konversi Linux

| Kebutuhan macOS | Padanan Linux | Catatan |
|---|---|---|
| ImageIO/CoreGraphics | crate `image` + ImageMagick | Native untuk format umum; ImageMagick untuk HEIC/AVIF/PDF/ekstraksi |
| AVFoundation | `ffmpeg` | Video/audio, termasuk GIF dan ekstraksi audio |
| PDFKit | `pdftoppm` / `pdftotext` (poppler) | Render halaman dan ekstraksi teks |
| Quick Look/Office | LibreOffice headless (`soffice --convert-to`) | Dokumen, slide, spreadsheet |
| EPUB tools | `pandoc` | EPUB/Markdown/HTML/DOCX |
| Archive Utility | `zip`, `unzip`, `tar`, `gzip` | Kompresi dan ekstraksi; tidak memakai shell |

Deteksi tool dilakukan sekali saat startup; target yang tidak tersedia tetap tampil dengan
alasan agar pengguna tahu dependensi yang perlu dipasang. Semua proses memakai
`Command` + argv tanpa shell. Konversi dijalankan di luar main thread dan hasilnya
dilaporkan lewat notifikasi desktop.

**Batas lingkup:** roda aksi file Tangerine (AirDrop, Float, Copy/Move, Terminal) sudah
diimplementasikan penuh. Gestur *drag + modifier* tidak bisa ditiru karena Wayland
menyembunyikan status drag dan posisi pointer global dari aplikasi biasa; sebagai gantinya
dipakai live-modifier switching: buka roda dulu (hotkey/menu layanan), lalu `Shift` /
`Super` / `Ctrl` mengganti mode secara instan sambil mempertahankan sumber file ter-seed.
Keputusan ini diambil bersama user lewat dialog pada 2026-10-25.
