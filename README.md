# RadialDeck

Overlay radial untuk Linux: konversi file dan buka folder proyek dari satu shortcut.

Tekan `Ctrl+Alt+Space`, atau jalankan `radial-deck` dengan file/folder sebagai argumen,
lalu pilih format pada roda. Drag file ke panel juga bisa; klik kanan file di Dolphin
untuk membuka roda langsung.

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
| Global hotkey via XDG GlobalShortcuts portal | ✅ |
| Navigasi keyboard: panah/scroll, angka `1`–`9`, `Enter`, `Esc` | ✅ |
| Roda folder proyek: filter real-time, multi-project, terminal, clipboard | ✅ |
| Manage folder, settings, autostart XDG, system tray | ✅ |
| Tema mengikuti sistem (KDE color scheme + GTK) / paksa Light / Dark | ✅ |
| Konfigurasi JSON atomik di `XDG_CONFIG_HOME` | ✅ |

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
| `radial-deck --hidden` | Mulai tersembunyi di tray (autostart) |
| `radial-deck --manage` | Jendela manajemen folder proyek |
| `radial-deck --settings` | Pengaturan hotkey, autostart, tema |
| `radial-deck --reset-config` | Hapus konfigurasi dan mulai ulang |
| `radial-deck --version` / `--help` | Versi / bantuan |

### Shortcut & interaksi roda

| Tombol | Aksi |
|---|---|
| `Ctrl+Alt+Space` | Toggle roda konversi (global) |
| Panah / scroll | Pilih format |
| `1`–`9` | Jalankan format ke-N |
| `Enter` | Terapkan format terpilih |
| `Esc` / klik area gelap | Tutup roda |
| Drag file ke panel | Ganti sumber, kategori mengikuti file pertama |
| Tab kategori | Ganti target; sumber yang tidak cocok ditandai |

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
Roda folder mendukung filter real-time, `1`–`9`, `Ctrl+Enter` (terminal), klik tengah
(salin path), dan `Tab`/panah untuk ganti project.

---

## Konfigurasi

Folder: `$XDG_CONFIG_HOME/radial-deck/config.json` (default `~/.config/radial-deck/config.json`).
Ditulis otomatis dan atomik; tidak perlu edit manual. Path konversi tidak disimpan.

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
        ├── wheel.rs           # roda folder (Cairo)
        ├── manage.rs / settings.rs / hotkey.rs / tray.rs
        └── theme.rs           # CSS, deteksi dark KDE/GTK
```

Diagram lengkap, threading, dan keputusan keamanan: [`docs/architecture.md`](docs/architecture.md).
Riset dan perbandingan stack: [`docs/research.md`](docs/research.md).

---

## Pengembangan

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all

# jalankan versi debug
cargo run -p radial-deck -- --show /path/ke/file.png
```

CI menjalankan fmt, clippy, test, dan build release di Ubuntu 24.04.

---

## Status Verifikasi

| Test | Hasil |
|---|---|
| Unit test | ✅ 41 lulus (`deck-core` 38, app 3) |
| fmt + clippy `-D warnings` | ✅ bersih |
| Layer-shell roda konversi | ✅ screenshot KDE Wayland |
| PNG → JPG via klik roda | ✅ file JPEG 1200×800 valid |
| WAV → MP3 via klik roda | ✅ MP3 1 detik, ffprobe valid |
| Arsip: ZIP/TAR/GZIP (file & folder), extract | ✅ smoke test engine + klik GZIP di roda |
| Hotkey global | ✅ `Ctrl+Alt+Space` membuka panel |
| Single instance + argumen file | ✅ proses kedua meneruskan file ke proses utama |
| Dismiss `Esc` / klik luar | ✅ diverifikasi diff screenshot |
| Idle RAM (release, hidden) | ~25 MB PSS / ~48 MB RSS (mayoritas shared GTK) |
| Binary release (stripped) | 6.3 MB |

---

## Keterbatasan

- Posisi panel di tengah output; Wayland tidak mengekspos posisi kursor global.
- Shortcut dipegang portal sehingga butuh persetujuan KDE satu kali.
- Konversi memerlukan tool eksternal; Flatpak sandbox belum dapat memakai binary host
  (manifest masih WIP, lihat `packaging/flatpak/`).
- File tools Tangerine (crop, trim, merge, dll.) belum diimplementasikan; fokus tahap ini
  adalah konversi format dan aksi arsip.

---

## Langkah Berikutnya

- **Windows:** core tetap; ganti layer-shell → Win32 topmost overlay; hotkey
  `RegisterHotKey`; konverter lewat ImageMagick/ffmpeg yang di-bundle.
- **macOS:** `NSPanel` + Carbon hotkey + `NSStatusItem`; konversi lewat `sips`/AVFoundation
  atau tool yang di-bundle.
- **File tools:** crop/trim/merge/compress dengan preview sebelum eksekusi.
- **Flatpak:** bundle konverter ke runtime atau gunakan `flatpak-spawn --host` dengan izin.

---

## Lisensi

MIT — lihat [LICENSE](LICENSE).

Referensi: [Tangerine](https://tangerineformac.com/) (produk/UX),
[ProjectDeck](https://github.com/nizar268/Shortcut-Floating) (arsitektur macOS).
Tidak ada kode yang disalin dari keduanya.
