# RadialDeck

Overlay radial untuk Linux dengan dua alur yang terpisah:

- **Quick actions** — `Ctrl+Alt+Space`: konversi file/arsip dan aksi file.
- **Project folders** — `Ctrl+Alt+P`: buka folder proyek favorit.

Jalankan `radial-deck` dengan file/folder sebagai argumen, drag file ke panel, atau klik
kanan file di Dolphin untuk langsung membuka roda konversi.

> Terinspirasi oleh [Tangerine for Mac](https://tangerineformac.com/) (UX roda konversi)
> dan [ProjectDeck](https://github.com/nizar268/Shortcut-Floating) (arsitektur launcher).
> Tidak ada kode yang disalin; implementasi asli, lisensi MIT.

---

## Fitur

| Fitur | Status |
|---|---|
| Roda konversi radial (Wayland layer-shell, Cairo) | ✅ |
| 10 kategori: gambar, video, audio, PDF, dokumen, slide, spreadsheet, EPUB, subtitle, arsip | ✅ |
| Drag & drop file ke panel; multi-file / mixed selection | ✅ |
| Format yang tidak tersedia ditandai redup + alasan (tooltip/status) | ✅ |
| Klik kanan file di Dolphin → **RadialDeck actions…** | ✅ |
| Single instance: argumen file diteruskan ke proses yang sudah berjalan | ✅ |
| Dua global hotkey via XDG GlobalShortcuts portal | ✅ |
| Navigasi keyboard: panah/scroll, angka `1`–`9`, `Enter`, `Esc` | ✅ |
| Roda folder proyek: filter real-time, multi-project, terminal, clipboard | ✅ |
| Kartu folder tunggal (tidak ada donut kosong) + roda untuk banyak folder | ✅ |
| Manage: rename, warna per folder/project, urutan, buka/terminal/salin, konfirmasi hapus | ✅ |
| Settings, autostart XDG, system tray, notifikasi | ✅ |
| Tema mengikuti sistem (KDE color scheme + GTK) / paksa Light / Dark | ✅ |
| Konfigurasi JSON atomik di `XDG_CONFIG_HOME`; migrasi schema | ✅ |
| Log harian di `XDG_STATE_HOME` (retensi 7 file) | ✅ |
| Renderer Cairo default: ringan, tidak membutuhkan GPU | ✅ |

---

## Kebutuhan Sistem

- **OS:** Linux (Arch/CachyOS diuji; Debian/Ubuntu kompatibel)
- **Desktop:** KDE Plasma 6 Wayland (target utama), X11 fallback
- **Wajib:** `gtk4`, `gtk4-layer-shell`, `xdg-desktop-portal`, `xdg-utils`
- **Konverter opsional** (roda mendeteksi otomatis; format terkait dinonaktifkan bila
  tool tidak ada):
  - `imagemagick` — HEIC/AVIF/PDF output, format gambar non-native
  - `ffmpeg` — video dan audio
  - `poppler` (`pdftoppm`, `pdftotext`) — PDF → gambar/teks
  - `libreoffice` — dokumen, slide, spreadsheet (ekspor EPUB memakai filter EPUB
    bawaan LibreOffice; build yang tidak menyertakannya akan melaporkan error)
  - `pandoc` — dokumen/EPUB → Markdown
  - Arsip (ZIP/TAR/GZIP) ditangani native oleh Rust, tanpa binary eksternal.
    GZIP pada satu file menghasilkan `.gz`; pada folder/arsip menghasilkan
    `.tar.gz`.

---

## Instalasi

### Cepat (user-local, tanpa sudo)

```bash
git clone https://github.com/ReyyIchiro/radialdeck.git
cd radial-deck
./scripts/install.sh
```

Script memasang binary ke `~/.local/bin`, desktop entry, ikon, dan service menu Dolphin.
Jalankan sekali `radial-deck --hidden` lalu setujui dialog *Global Shortcuts* KDE.

### Arch / CachyOS (AUR)

```bash
cd packaging/aur
makepkg -si
```

### Build manual

```bash
# Arch
sudo pacman -S gtk4 gtk4-layer-shell xdg-desktop-portal rust

cargo build --release --locked
install -Dm755 target/release/radial-deck ~/.local/bin/radial-deck
```

---

## Penggunaan

| Perintah | Aksi |
|---|---|
| `radial-deck FILE...` | Buka roda konversi untuk file/drag selection |
| `radial-deck --show` | Buka roda konversi kosong |
| `radial-deck --projects` | Buka roda folder proyek |
| `radial-deck --hidden` | Mulai tersembunyi di tray (autostart) |
| `radial-deck --manage` | Jendela manajemen folder proyek |
| `radial-deck --settings` | Pengaturan hotkey, autostart, tema |
| `radial-deck --reset-config` | Hapus konfigurasi dan mulai ulang |
| `radial-deck --version` / `--help` | Versi / bantuan |

### Shortcut & interaksi roda

| Tombol | Aksi |
|---|---|
| `Ctrl+Alt+Space` | Toggle roda quick actions (global) |
| `Ctrl+Alt+P` | Toggle roda project folders (global) |
| Panah / scroll | Pilih format atau folder |
| `1`–`9` | Jalankan item ke-N |
| `Enter` | Terapkan format / buka folder |
| `Esc` / klik area gelap | Tutup roda (state tetap konsisten) |
| Drag file ke panel | Ganti sumber, kategori mengikuti file pertama |
| Tab kategori | Ganti target; sumber yang tidak cocok ditandai |
| `Tab` di roda folder | Ganti project (bila ada lebih dari satu) |
| `Ctrl+Enter` di roda folder | Buka folder di terminal |
| Klik tengah di roda folder | Salin path folder |
| Klik kanan item | Menu konteks (buka, terminal, salin, edit, hapus) |

Konversi berjalan di thread terpisah; roda langsung tertutup dan hasil ditulis **di
folder yang sama** dengan file asli (nama bentrok tidak pernah ditimpa). Notifikasi
menampilkan ringkasan sukses/gagal.

### Format

| Kategori | Target |
|---|---|
| Images | JPG, PNG, WEBP, HEIC, TIFF, AVIF, BMP, PDF |
| Video | MP4, MOV, MKV, WEBM, AVI, GIF, WMV |
| Audio | MP3, M4A, WAV, FLAC, OGG, OPUS, AIFF, WMA |
| PDF | DOCX, PPTX, XLSX, JPG, PNG, EPUB, TXT, MD |
| Documents | PDF, DOCX, ODT, TXT, MD, EPUB, HTML, RTF |
| Slides | PDF, PPTX, ODP, JPG, PNG |
| Spreadsheets | PDF, XLSX, ODS, CSV, TSV |
| EPUB | PDF, HTML, DOCX, MD, TXT |
| Subtitles | SRT, VTT, TXT |
| Archives | ZIP, TAR, GZIP, EXTRACT |

Beberapa target memerlukan tool eksternal; roda menampilkan alasan bila tidak tersedia.

### Folder proyek

Tray → **Manage Folders** (`--manage`): tambah folder lewat drag-drop atau file chooser.
Folder punya nama, path, warna opsional, dan urutan yang bisa diubah. Project juga punya
warna sendiri; keduanya dipakai di roda. Tombol **Open Wheel** menampilkan hasilnya
langsung.

Roda folder mendukung filter real-time, `1`–`9`, `Ctrl+Enter` (terminal), klik tengah
(salin path), klik kanan (menu), dan `Tab` untuk ganti project. Jika hanya ada satu folder,
roda berubah menjadi satu kartu besar agar tidak ada cincin kosong.

---

## Konfigurasi

Folder: `$XDG_CONFIG_HOME/radial-deck/config.json` (default `~/.config/radial-deck/config.json`).
Ditulis otomatis dan atomik; tidak perlu edit manual. Path konversi tidak disimpan.

Schema saat ini **2**; file schema 1 dibaca sebagai schema 2 saat load dengan shortcut
project default (`projects_hotkey`), lalu ditulis sebagai schema 2 pada perubahan config
berikutnya. File yang lebih baru ditolak dengan pesan jelas, bukan ditimpa. Detail invariant ada di [`docs/architecture.md`](docs/architecture.md).

Log: `$XDG_STATE_HOME/radial-deck/logs/radial-deck.YYYY-MM-DD.log`
(fallback `~/.local/state/...`), retensi 7 hari. Buka lewat **Settings → Open Logs** atau
set `RUST_LOG=radial_deck=debug` untuk diagnosis.

Renderer default adalah **Cairo** (software) agar memori rendah dan tidak bergantung pada
GPU NVIDIA/Intel. Override dengan `GSK_RENDERER=gl` bila ingin renderer GTK lain.

---

## Arsitektur

```text
crates/
├── deck-core/                 # tanpa UI: model folder, search, store, konversi
│   └── src/convert/
│       ├── registry.rs        # kategori, target, availability, tool detection
│       ├── image.rs           # native image + ImageMagick
│       ├── external.rs        # ffmpeg, pdftoppm, LibreOffice, pandoc
│       ├── archive.rs         # zip/tar/gzip
│       └── mod.rs             # orkestrasi batch + Outcome
└── radial-deck/               # shell Linux
    └── src/
        ├── action_wheel.rs    # roda konversi Cairo: gambar, hit-test, input
        ├── action_launcher.rs # panel overlay, tab kategori, drag & drop
        ├── controller.rs      # state, CLI single-instance, thread konversi
        ├── launcher.rs        # roda folder proyek
        ├── wheel.rs           # roda folder (Cairo), kartu item tunggal
        ├── manage.rs          # CRUD project/folder, warna, urutan
        ├── settings.rs        # dua shortcut, autostart, terminal, tema, log
        ├── hotkey.rs          # GlobalShortcuts portal (dua binding)
        ├── tray.rs            # StatusNotifierItem
        ├── logging.rs         # log harian XDG
        └── theme.rs           # CSS, deteksi dark KDE/GTK
```

Diagram lengkap, threading, dan keputusan keamanan: [`docs/architecture.md`](docs/architecture.md).
Riset dan perbandingan stack: [`docs/research.md`](docs/research.md).

---

## Pengembangan

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# jalankan versi debug
cargo run -p radial-deck -- --show /path/ke/file.png
```

CI menjalankan fmt, clippy, test, dan build release di Ubuntu 24.04.

---

## Status Verifikasi

| Test | Hasil |
|---|---|
| Unit test | ✅ 43 lulus (`deck-core` 40, app 3) |
| fmt + clippy `-D warnings` | ✅ bersih |
| Layer-shell roda konversi + roda folder | ✅ screenshot KDE Wayland |
| Dua hotkey portal | ✅ `Ctrl+Alt+Space` dan `Ctrl+Alt+P` aktif bersamaan |
| Kartu folder tunggal + panel kompak | ✅ klik kartu membuka Dolphin, panel tertutup |
| Manage 3 project / 3 folder | ✅ rename, warna, add folder, badge missing |
| Project color + folder color | ✅ popover palet, tersimpan ke config |
| Dismiss `Esc` / klik luar / reopen hotkey | ✅ window dan state controller konsisten |
| PNG → JPG via klik roda | ✅ file JPEG valid |
| WAV → MP3 via klik roda | ✅ ffprobe valid |
| Arsip: ZIP/TAR/GZIP (file & folder), extract | ✅ smoke test engine + klik roda |
| Single instance + argumen file | ✅ proses kedua meneruskan file ke proses utama |
| Idle RAM (release, hidden, Cairo) | 53 MB RSS saat start |
| RAM setelah window pertama | 67–87 MB RSS (Cairo) |
| Binary release | 6.5 MiB (stripped) |

---

## Keterbatasan

- Posisi panel di tengah output; Wayland tidak mengekspos posisi kursor global.
- Shortcut dipegang portal sehingga butuh persetujuan KDE satu kali saat pertama kali
  binding project ditambahkan.
- Konversi memerlukan tool eksternal; Flatpak sandbox belum dapat memakai binary host
  (manifest masih WIP, lihat `packaging/flatpak/`).
- File tools Tangerine (crop, trim, merge, dll.) belum diimplementasikan; fokus tahap ini
  adalah konversi format dan aksi arsip.

---

## Langkah Berikutnya

Lihat [`docs/roadmap.md`](docs/roadmap.md) untuk keputusan dan kriteria tiap platform.
Ringkasnya: Windows memakai shell Tauri v2 + WebView2 dengan backend `deck-core`; macOS
memakai AppKit/SwiftUI native + `NSPanel`/Carbon hotkey. File tools dengan preview dan
Flatpak adalah pekerjaan Linux berikutnya.

---

## Lisensi

MIT — lihat [LICENSE](LICENSE).

Referensi: [Tangerine](https://tangerineformac.com/) (produk/UX),
[ProjectDeck](https://github.com/nizar268/Shortcut-Floating) (arsitektur macOS).
Tidak ada kode yang disalin dari keduanya.
