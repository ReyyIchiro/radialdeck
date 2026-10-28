# RadialDeck

Overlay radial untuk Linux dengan dua alur yang terpisah:

- **Quick actions** — `Ctrl+Alt+Space`: konversi file/arsip dan aksi file.
- **Project folders** — `Ctrl+Alt+Q`: buka folder proyek favorit.

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
| Tiga mode roda: **Convert** (`Shift`), **Tools** (`Super`), **Actions** (`Ctrl`) | ✅ |
| 26 file tools lokal: compress, crop, edit, annotate, frame, redact, PDF merge/split/organize, video trim/crop/split/speed/snapshot/join/mute/redact, audio trim/normalize/channel/redact/visualizer | ✅ |
| Form parameter per tool dengan validasi dan nilai default | ✅ |
| Aksi file: Copy, Move, Copy Path, Terminal, Send (KDE Connect), Float | ✅ |
| Float shelf: jendela mengambang untuk menampung file, tiap baris bisa di-drag keluar | ✅ |
| Drag & drop file ke panel; multi-file / mixed selection | ✅ |
| Format yang tidak tersedia ditandai redup + alasan (tooltip/status) | ✅ |
| Klik kanan file di Dolphin → **RadialDeck actions…** | ✅ |
| Single instance: argumen file diteruskan ke proses yang sudah berjalan | ✅ |
| Dua global hotkey via XDG GlobalShortcuts portal | ✅ |
| Navigasi keyboard: panah/scroll, angka `1`–`9`, `Enter`, `Esc` | ✅ |
| Roda folder proyek: filter real-time, multi-project, terminal, clipboard | ✅ |
| Target folder tunggal (cincin penuh) + roda untuk banyak folder | ✅ |
| Rail project: tab aktif, overflow `+N`, palet project/folder | ✅ |
| Manage: rename, warna per folder/project, urutan, buka/terminal/salin, konfirmasi hapus | ✅ |
| Settings, autostart XDG, system tray, notifikasi | ✅ |
| Tema mengikuti sistem (KDE color scheme + GTK) / paksa Light / Dark | ✅ |
| Konfigurasi JSON atomik di `XDG_CONFIG_HOME`; migrasi schema | ✅ |
| Log harian di `XDG_STATE_HOME` (retensi 7 file) | ✅ |
| Progres langsung + tombol **Cancel** untuk tool/konversi panjang | ✅ |
| Lokasi output: di samping file, subfolder, atau satu folder tetap | ✅ |
| Urutan & visibilitas tool per kategori (Settings) | ✅ |
| Riwayat operasi + Undo dari tray | ✅ |
| CLI batch `radial-deck convert` tanpa membuka UI | ✅ |
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
  - `ghostscript` — kompres PDF
  - `7zip` — ekstrak arsip 7Z/RAR
  - `kdeconnect` — aksi **Send** (opsional, tanpa ini wedge redup)
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
Jalankan `radial-deck --hidden` sekali, lalu setujui dialog *Global Shortcuts* KDE.
Aktifkan **Launch at login** di Settings supaya hotkey tetap ada setelah reboot; kalau
hotkey diam saja, jalankan `./scripts/doctor.sh`.

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
| `radial-deck --convert FILE...` | Paksa mode Convert |
| `radial-deck --tools FILE...` | Buka langsung di mode Tools |
| `radial-deck --actions FILE...` | Buka langsung di mode Actions |
| `radial-deck --show` | Buka roda konversi kosong |
| `radial-deck --projects` | Buka roda folder proyek |
| `radial-deck --hidden` | Mulai tersembunyi di tray (autostart) |
| `radial-deck --manage` | Jendela manajemen folder proyek |
| `radial-deck --settings` | Jendela pengaturan bersidebar: hotkey, tema, output, tool, log |
| `radial-deck --reset-config` | Hapus konfigurasi dan mulai ulang |
| `radial-deck convert --to webp FILES…` | Konversi batch headless, cetak path hasil |
| `radial-deck --version` / `--help` | Versi / bantuan |

Subcommand batch tidak memuat GTK sama sekali, jadi bisa dipakai lewat SSH:

```bash
radial-deck convert --to webp --into ~/hasil foto1.png foto2.png
radial-deck convert --to mp4 --subfolder Converted clip.mov
```

Path hasil dicetak satu per baris (kecuali `--quiet`); kegagalan per file ke stderr.
Exit code `0` hanya bila semua file berhasil, `2` untuk argumen/target yang tidak valid.

### Shortcut & interaksi roda

| Tombol | Aksi |
|---|---|
| `Ctrl+Alt+Space` | Toggle roda quick actions (global) |
| `Ctrl+Alt+Q` | Toggle roda project folders (global) |
| `Shift` / `Super` / `Ctrl` | Ganti mode Convert / Tools / Actions |
| Panah / scroll | Pilih format, tool, aksi, atau folder |
| `1`–`9` | Jalankan item ke-N |
| `Enter` | Terapkan format / buka folder |
| `Esc` / klik area gelap | Tutup roda (state tetap konsisten) |
| Drag file ke panel | Ganti sumber, kategori mengikuti file pertama |
| Tab kategori | Ganti target; sumber yang tidak cocok ditandai |
| Klik tab project | Ganti project; tab aktif dan fokus keyboard dibedakan |
| `Tab` di roda folder | Pindah fokus ke rail project; `Enter` memilih project |
| `Ctrl+Enter` di roda folder | Buka folder di terminal |
| Klik tengah wedge | Salin path folder ke clipboard |
| Klik tengah pusat | Kelola project (atau buka folder saat hanya satu) |
| Klik kanan item | Menu konteks (buka, terminal, salin, edit, hapus) |

Konversi dan tool berjalan di thread terpisah; roda langsung tertutup. Panel progres kecil
muncul di pojok kanan bawah (layer-shell, tanpa mengambil fokus keyboard) dan menampilkan
file yang sedang diproses plus output mentah FFmpeg/Ghostscript. Tombol **Cancel**
menghentikan proses anak dan menghapus output parsial yang sudah terdaftar. Notifikasi
menampilkan ringkasan sukses/gagal. Hasilnya juga masuk riwayat tray
(**Recent Operations → Undo …**): Undo hanya menghapus file yang dibuat operasi itu, jadi
file lama tidak pernah ikut terhapus. Riwayat disimpan terpisah di
`$XDG_STATE_HOME/radial-deck/history.json` (maksimal 30 entri, tidak mengotori config).

Default output adalah **folder yang sama** dengan file asli dan nama bentrok tidak pernah
ditimpa (`foto.webp`, `foto-1.webp`, …). Lokasi output bisa diubah di **Settings → Output** menjadi subfolder (default `RadialDeck`) atau satu folder tetap.

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

### Tools

Tekan `Super` (atau klik tab **Tools**) untuk mengganti isi roda dengan tool yang berlaku
untuk kategori file aktif. Tool yang butuh parameter membuka form di dalam panel; `Enter`
menjalankan, `Esc` kembali ke roda. Selama form terbuka, `Ctrl`/`Shift`/`Super` tetap
menjadi modifier teks dan tidak mengganti mode.

Semua tool memakai kebijakan lokasi output yang sama dengan konversi, memakai nama unik,
dan tidak pernah menimpa file yang ada. Contoh: **Compress** (kualitas gambar / CRF video / bitrate
audio / preset PDF), **Crop**, **Edit** (brightness, contrast, saturation, rotate, flip),
**Annotate**, **Frame**, **Redact**, **Create PDF**, **Merge PDFs**, **Split PDF**,
**Organize PDF**, **Join Videos**, **Trim**, **Crop Video**, **Change Speed**,
**Snapshots**, **Split Video**, **Mute**, **Normalize Audio**, **Channels**,
**Audio → Video**, **Extract Archive**.

Urutan tool di roda bisa diatur per kategori lewat **Settings → Tool wheel**: saklar
menentukan tool tampil atau tidak, panah menggeser urutannya, dan urutan itu langsung
menentukan digit `1`–`9` saat roda dibuka. Mematikan semua tool untuk satu kategori
disengaja dan tidak dikembalikan ke default secara diam-diam.

### Actions

Tekan `Ctrl` (atau klik tab **Actions**):

| Aksi | Perilaku |
|---|---|
| Copy | Menyalin file sebagai `text/uri-list`, bisa di-paste di Dolphin |
| Move | Memindahkan file ke folder pilihan (chooser) |
| Copy Path | Menyalin path absolut ke clipboard |
| Terminal | Membuka terminal di folder file pertama |
| Send | Mengirim lewat KDE Connect; redup bila tidak ada perangkat |
| Float | Menampilkan *Float shelf* berisi file tersebut |

Float shelf adalah jendela mengambang 340×420 di pojok kanan atas: daftar file, tombol tutup
per baris, dan setiap baris adalah drag source sungguhan sehingga bisa ditarik kembali ke
Dolphin atau aplikasi lain. Di Wayland ia memakai layer-shell (`OnDemand`), di X11 menjadi
jendela biasa.

### Folder proyek

Tray → **Manage Folders** (`--manage`): tambah folder lewat drag-drop atau file chooser.
Folder punya nama, path, warna opsional, dan urutan yang bisa diubah. Project juga punya
warna sendiri; keduanya dipakai di roda. Tombol **Open Wheel** menampilkan hasilnya
langsung.

Rail di atas roda menampilkan maksimal lima project sebagai tab; sisanya masuk menu `+N`.
Tab aktif ditandai border oranye, sedangkan fokus keyboard memakai border putih agar tidak
ambigu. Klaster project memakai warna project, dan warna folder (jika ada) menimpa warna
wedge.

Roda folder mendukung filter real-time, `1`–`9`, `Ctrl+Enter` (terminal), klik tengah
(salin path), klik kanan (menu), serta klik tab untuk ganti project. Jika hanya ada satu
folder, roda menjadi satu target cincin penuh tanpa sektor kosong; pusat membuka folder,
dan saat ada beberapa folder pusat membuka Manage.

---

## Konfigurasi

Folder: `$XDG_CONFIG_HOME/radial-deck/config.json` (default `~/.config/radial-deck/config.json`).
Ditulis otomatis dan atomik; tidak perlu edit manual. Path konversi tidak disimpan.

Schema saat ini **5**. File lama dimigrasikan saat load: shortcut project default lama
`CTRL+ALT+P` menjadi `CTRL+ALT+Q`, dan schema 4 mendapat preferensi output serta daftar
tool per kategori dengan nilai default. File yang lebih baru ditolak dengan pesan jelas,
bukan ditimpa.

Kunci baru di `preferences`:

| Kunci | Arti |
|---|---|
| `output_location` | `"alongside"`, `{"subfolder": "RadialDeck"}`, atau `{"folder": "/path"}` |
| `output_subfolder` | Nama subfolder yang dipakai mode `subfolder` |
| `wheel_tools` | Peta `kategori → [id tool]`; urutan menentukan digit `1`–`9`, daftar kosong menyembunyikan semua |

Di KDE, portal menyimpan binding yang sudah pernah disetujui pengguna. Jika binding lama
masih `Ctrl+Alt+P`, buka **Settings → Shortcuts → Change Shortcuts…** sekali dan pilih `Ctrl+Alt+Q`;
setelah itu binding baru dipakai. Install baru langsung memakai `Ctrl+Alt+Q`. Detail invariant ada di [`docs/architecture.md`](docs/architecture.md).

Log: `$XDG_STATE_HOME/radial-deck/logs/radial-deck.YYYY-MM-DD.log`
(fallback `~/.local/state/...`), retensi 7 hari. Buka lewat **Settings → Advanced → Open Logs** atau
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
│   ├── tools/                 # 26 file tool lokal (image, media, pdf, shell)
│   ├── job.rs                 # scope output policy, progres, cancel, cleanup
│   ├── history.rs             # riwayat operasi + Undo
│   └── file_actions.rs        # copy/move/trash helpers tanpa shell
└── radial-deck/               # shell Linux
    └── src/
        ├── action_wheel.rs    # roda konversi Cairo: gambar, hit-test, input
        ├── action_launcher.rs # panel overlay, tab kategori, mode, form, drag & drop
        ├── float_shelf.rs     # jendela Float + baris drag source
        ├── surface.rs         # deteksi layer-shell + penempatan monitor
        ├── controller.rs      # state, CLI single-instance, thread konversi
        ├── launcher.rs        # overlay roda folder + rail project
        ├── wheel.rs           # roda folder (Cairo), target tunggal, menu
        ├── project_tabs.rs    # tab project, overflow +N, sinkronisasi state
        ├── widgets.rs         # helper tombol ikon dan klaster warna
        ├── manage.rs          # CRUD project/folder, warna, urutan
        ├── progress_window.rs # panel progres layer-shell tanpa fokus + Cancel
        ├── batch.rs           # subcommand `convert` headless
        ├── settings.rs        # sidebar 6 halaman: shortcut, tema, output, tool, log
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

### Uji UI tanpa mengganggu sesi

`scripts/ui-smoke.sh` menjalankan aplikasi di dalam sesi KWin virtual
(`kwin_wayland --virtual`), jadi overlay, layer-shell, dan Float shelf dirender ke
framebuffer offscreen. Layar, pointer, dan fokus keyboard Anda tidak tersentuh.

```bash
cargo build --release
scripts/ui-smoke.sh          # 40 pemeriksaan, exit 1 bila ada yang gagal
```

Enam lane yang diuji: **wayland** (overlay benar-benar ter-map dan tergambar, dibuktikan
screenshot), **x11** (form tool, Float shelf, `Esc`, live-modifier, lewat Xwayland
bersarang), **settings** (Down menelusuri keenam halaman, tiap halaman difoto),
**projects** (daftar folder dari config yang di-seed), **dark** (tema gelap benar-benar
menggelapkan jendela), dan **CLI batch** (tanpa GTK sama sekali).
Butuh `kwin_wayland`, `spectacle`, `xdotool`, dan `magick`. Screenshot serta log tersimpan
di `/tmp/radial-deck-ui-smoke` (ubah dengan `RADIAL_DECK_SMOKE_OUT`).

Skrip ini juga membuat bus D-Bus sesi sendiri, jadi tray, notifikasi, portal, dan KDE
Connect selama tes tidak menyentuh desktop Anda. Semua proses yang lahir di bus itu
dimatikan saat selesai — termasuk service yang sempat daemonize — sehingga skrip keluar
tanpa menggantung pipe pemanggilnya.

---

## Status Verifikasi

| Test | Hasil |
|---|---|
| Unit test | ✅ 85 lulus (`deck-core` 69, app 16) |
| fmt + clippy `-D warnings` | ✅ bersih |
| Layer-shell roda konversi + roda folder | ✅ screenshot KDE Wayland |
| Dua hotkey portal | ✅ `Ctrl+Alt+Space` dan `Ctrl+Alt+Q` aktif bersamaan |
| Target folder tunggal (cincin penuh) | ✅ klik target membuka Dolphin, panel tertutup |
| Rail project overflow `+3` | ✅ screenshot: 5 tab + popover, tab aktif vs fokus keyboard |
| Mode tanpa hasil filter | ✅ sektor disembunyikan, caption “No matches” |
| Manage 8 project / 4 folder | ✅ rail, rename, warna, add folder, urutan, badge missing |
| Project color + folder color | ✅ popover palet, tersimpan ke config |
| Dismiss `Esc` / klik luar / reopen hotkey | ✅ window dan state controller konsisten |
| PNG → JPG via klik roda | ✅ file JPEG valid |
| WAV → MP3 via klik roda | ✅ ffprobe valid |
| Arsip: ZIP/TAR/GZIP (file & folder), extract | ✅ smoke test engine + klik roda |
| Single instance + argumen file | ✅ proses kedua meneruskan file ke proses utama |
| Idle RAM (release, hidden, Cairo) | 53 MB RSS saat start |
| RAM setelah window pertama | 67–87 MB RSS (Cairo) |
| 26 tool: smoke engine (`deck-core/examples/tool_smoke`) | ✅ 24 sukses, 2 kegagalan input yang memang ditolak |
| Form tool: Compress PNG kualitas 60 | ✅ `photo-compressed.png` 96×64 ditulis dari form |
| Mode Actions: Copy Path | ✅ clipboard berisi path absolut file |
| Mode Actions: Float | ✅ shelf 340×420 berisi baris `photo.png` |
| Roda Tools di Wayland | ✅ screenshot: 8 tool gambar + caption |
| Form menahan `Ctrl`/`Shift`/`Super` | ✅ `Ctrl+A` memilih teks, mode tidak berpindah |
| Overlay X11 mengikuti monitor kursor | ✅ 1920×1080 penuh di eDP-1, bukan monitor lain |
| Layer-shell di-anchor ke monitor kursor | ✅ roda muncul di monitor tempat pointer berada |
| Progres + Cancel | ✅ stream stderr FFmpeg, output parsial dibersihkan, proses tetap hidup |
| Lokasi output (subfolder / folder tetap) | ✅ unit test + konversi nyata ke folder terpisah |
| Urutan tool kustom per kategori | ✅ unit test; daftar kosong menyembunyikan semua tool |
| Riwayat + Undo | ✅ entri tersimpan, Undo menghapus output dan memperbarui tray |
| CLI batch `convert` | ✅ exit 0/2, nama bentrok `-1`, tanpa inisialisasi GTK (jalan di SSH) |
| `scripts/ui-smoke.sh` (sesi KWin virtual) | ✅ 40/40 lulus tanpa menyentuh layar nyata |
| Portal tanpa `GlobalShortcuts` saat login | ✅ retry backoff + drop-in systemd, hotkey pulih sendiri |
| Binary release | 6.9 MiB (stripped) |

---

## Troubleshooting

### Hotkey tidak bereaksi

Gejala: aplikasi berjalan, roda bisa dibuka lewat tray, tetapi `Ctrl+Alt+Space` dan
`Ctrl+Alt+Q` tidak melakukan apa pun.

Penyebab paling umum di KDE Plasma: `xdg-desktop-portal` diaktifkan D-Bus **sebelum**
Plasma mengekspor `XDG_CURRENT_DESKTOP` ke systemd user manager. Portal lalu tidak bisa
mencocokkan `UseIn=KDE` pada `kde.portal`, jatuh ke backend GTK, dan backend itu tidak
mengimplementasikan `GlobalShortcuts`. Cara tercepat memeriksa sekaligus memperbaiki:

```bash
./scripts/doctor.sh          # laporan saja
./scripts/doctor.sh --fix    # restart portal kalau antarmukanya hilang
```

Pemeriksaan manualnya:

```bash
busctl --user introspect org.freedesktop.portal.Desktop \
  /org/freedesktop/portal/desktop | grep GlobalShortcuts
```

Tidak ada keluaran berarti portal memang tidak menawarkan antarmuka tersebut. RadialDeck
sudah mencoba ulang dengan backoff (2s → 60s) dan menulis alasannya di log, tetapi
memperbaiki portalnya membuat hotkey langsung hidup:

```bash
systemctl --user restart xdg-desktop-portal
```

Aplikasi juga memantau nama bus portal, jadi kalau portal di-restart saat aplikasi
berjalan, sesi didaftarkan ulang sendiri dalam hitungan detik — tidak perlu menutup
RadialDeck. Log akan menampilkan `global shortcuts unavailable: the desktop portal
restarted; retrying in 2s` lalu `shortcut … active` lagi.

Agar tidak berulang setiap login, pin desktop untuk layanan portal:

```bash
mkdir -p ~/.config/systemd/user/xdg-desktop-portal.service.d
printf '[Service]\nEnvironment=XDG_CURRENT_DESKTOP=KDE\n' \
  > ~/.config/systemd/user/xdg-desktop-portal.service.d/desktop-env.conf
systemctl --user daemon-reload
systemctl --user restart xdg-desktop-portal
```

Kalau `GlobalShortcuts` **ada** tetapi tetap tidak ada reaksi, cek apakah binding pernah
ditolak: buka Settings → **Shortcuts** → *Change Shortcuts…* lalu set ulang. KDE menyimpan binding yang
ditolak tanpa trigger, sehingga `BindShortcuts` sukses tetapi tidak ada tombol yang
dipegang.

### Hotkey hilang setelah reboot

Aplikasi harus berjalan untuk memegang shortcut. Aktifkan **Launch at login** di Settings,
atau pastikan `~/.config/autostart/io.github.raihan.RadialDeck.desktop` ada dan berisi
`Exec="…/radial-deck" --hidden`.

---

## Keterbatasan

- Wayland tidak mengekspos posisi kursor global, jadi roda membuka di tengah monitor yang
  berisi pointer (bukan di titik kursor). X11 fallback mengisi monitor tersebut.
- Gestur Tangerine (drag + tekan modifier, roda muncul dan tetap sampai drop) tidak dapat
  ditiru penuh: Wayland melarang aplikasi biasa melihat status drag lintas-aplikasi atau
  posisi pointer global. Sebagai gantinya, **live-modifier switching** dipakai — roda dibuka
  dulu (hotkey atau menu layanan), lalu `Shift` / `Super` / `Ctrl` mengganti mode dengan cepat.
  Lihat baris "Live-modifier switching" di Settings.
- Shortcut dipegang portal sehingga butuh persetujuan KDE satu kali saat pertama kali
  binding project ditambahkan.
- Konversi dan tools memerlukan tool eksternal; Flatpak sandbox belum dapat memakai binary
  host (manifest masih WIP, lihat `packaging/flatpak/`).
- Tool berjalan sinkron di thread kerja; file besar membuat status "running" bertahan
  sampai proses eksternal selesai.
- Drag keluar dari baris Float bergantung pada file manager tujuan yang menerima
  `text/uri-list`.

---

## Langkah Berikutnya

Lihat [`docs/roadmap.md`](docs/roadmap.md) untuk keputusan dan kriteria tiap platform.
Ringkasnya: Windows memakai shell Tauri v2 + WebView2 dengan backend `deck-core`; macOS
memakai AppKit/SwiftUI native + `NSPanel`/Carbon hotkey. Preview sebelum eksekusi tool dan
Flatpak adalah pekerjaan Linux berikutnya.

---

## Lisensi

MIT — lihat [LICENSE](LICENSE).

Referensi: [Tangerine](https://tangerineformac.com/) (produk/UX),
[ProjectDeck](https://github.com/nizar268/Shortcut-Floating) (arsitektur macOS).
Tidak ada kode yang disalin dari keduanya.
