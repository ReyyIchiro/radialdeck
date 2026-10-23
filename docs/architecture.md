# Arsitektur

## Gambaran

```text
┌──────────────────────────── radial-deck (GTK4, main thread) ───────────────────────────┐
│ action_launcher.rs + action_wheel.rs (konversi, Cairo)                                 │
│ launcher.rs + wheel.rs (folder, kartu tunggal)   manage.rs   settings.rs                │
│        ▲                     ▲                              ▲            ▲              │
│                     controller.rs (state, CLI, actions, windows)                        │
│        │                     │                              │            │              │
└────────┼─────────────────────┼──────────────────────────────┼────────────┼──────────────┘
         │ async-channel       │                              │            │
┌────────┴─────────┐  ┌────────┴─────────┐          ┌─────────┴────────┐  ┌┴──────────────┐
│ hotkey.rs        │  │ tray.rs          │          │ autostart.rs     │  │ actions.rs    │
│ tokio + ashpd    │  │ ksni (SNI)       │          │ XDG autostart    │  │ xdg-open,     │
│ GlobalShortcuts  │  │ own thread       │          │                  │  │ terminal,     │
│ portal           │  │                  │          │                  │  │ clipboard     │
└──────────────────┘  └──────────────────┘          └──────────────────┘  └───────────────┘
         │
┌────────┴────────────────────────────────────────────────────────────────────────────────┐
│ deck-core: Config/Project/Folder, search, Store (XDG, atomic save)                      │
│ convert::registry (kategori, target, availability), convert::* (image/ffmpeg/office)    │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

## Workspace

| Crate | Tanggung jawab | Dependensi UI/OS |
|---|---|---|
| `crates/deck-core` | model, validasi, pencarian, persistensi atomik, registry konversi + eksekusi tool | tidak ada (hanya `std::process::Command`) |
| `crates/radial-deck` | shell Linux: GTK4, layer-shell, portal, tray, autostart, roda konversi | ya (Linux) |

Core tidak mengetahui GTK, Wayland, XDG portal, atau path config default selain lewat
`default_config_path()`. Untuk port Windows/macOS, buat crate app baru yang memakai
`deck-core`; kontrak platform minimum: hotkey global, jendela overlay, tray, buka folder,
clipboard, notifikasi, autostart.

## Alur hotkey

1. `hotkey.rs` membuat satu sesi `org.freedesktop.portal.GlobalShortcuts` dengan dua
   binding: `toggle` (quick actions, default `Ctrl+Alt+Space`) dan `projects` (folder,
   default `Ctrl+Alt+P`).
2. Portal (KDE) menyimpan persetujuan user per binding; sinyal `Activated(id)` membawa id
   sehingga controller memilih overlay yang benar. Binding tidak memakai `XGrabKey`/Carbon;
   aman di Wayland dan tidak mencuri input saat aplikasi lain fokus.
3. `HotkeyEvent::Bound` memperbarui label aktual di Settings/tray; `Error` menyimpan pesan
   per-id agar satu binding gagal tidak menyembunyikan binding lain.
4. `ConfigureShortcuts` membuka UI portal; setelah selesai, `ListShortcuts` dibaca ulang.

Jika portal tidak tersedia, Settings menampilkan error yang dapat ditindaklanjuti
(mis. `xdg-desktop-portal` tidak berjalan). Aplikasi tetap bisa dipakai lewat tray.

## Overlay Wayland

- `gtk4_layer_shell::init_for_window` + layer `Overlay`, anchor ke keempat sisi,
  `KeyboardMode::Exclusive`, `namespace=radial-deck`, `exclusive_zone=-1`.
- Surface menutupi satu output (output aktif dipilih KWin). Latar transparan digambar
  sendiri: backdrop gelap + wheel, sehingga klik di luar wheel dapat menutup panel.
- Wayland tidak mengizinkan query posisi pointer global. Wheel diposisikan di tengah
  output. Di X11 (fallback) aplikasi memakai jendela biasa 760×760; posisi kursor belum
  dipakai karena GTK4 tidak menyediakan move window lintas backend — dicatat sebagai
  keterbatasan.
- Keyboard exclusive memberi fokus penuh saat panel tampil; `Esc` selalu menutup.
- **Invariant lifecycle:** satu-satunya pemilik visibilitas adalah controller/launcher.
  Handler klik luar dan `Esc` mengirim `Dismiss` ke controller, bukan memanggil
  `set_visible(false)` sendiri. Sebelumnya handler menyembunyikan root window tanpa
  memperbarui flag `visible`, sehingga hotkey berikutnya hanya “menutup” overlay tak
  terlihat — bug ini sudah diperbaiki dan diverifikasi.
- Overlay disembunyikan saat dismiss dan dipakai ulang agar pembukaan berikutnya cepat.
  Membuka overlay lain selalu menutup yang lama; manage/settings menutup keduanya.
  Pengelolaan window tetap memakai referensi controller; bila window destroyed (mis.
  compositor/restart), controller membuat ulang saat diminta.
- Bila surface gagal di-map, controller melakukan verifikasi state + retry satu kali.

## Roda konversi

- Dua entry point berbagi `ActionLauncher`: CLI/`dolphin` service menu (`radial-deck FILE...`)
  dan drag & drop. Instance kedua meneruskan argv ke instance utama lewat
  `GApplicationFlags::HANDLES_COMMAND_LINE`; tidak ada daemon tambahan.
- `Cli::parse` memvalidasi path (harus ada; path relatif dinormalisasi terhadap CWD).
  Kategori diambil dari file pertama; sumber tetap disimpan semua.
- `convert::registry::targets()` menghasilkan daftar target tetap per kategori. Setiap
  target membawa `reason` bila tidak tersedia; UI menampilkannya sebagai wedge redup,
  strikethrough, tooltip, dan status saat diklik.
- Tool eksternal dideteksi sekali saat startup dengan memindai `PATH` plus fallback
  `/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin` (desktop launch sering
  tidak memasukkan sbin, padahal Arch menaruh `ffmpeg`/`magick`/`soffice` di sana).
- Klik target yang tersedia: panel langsung disembunyikan, sumber difilter sesuai kategori,
  lalu `std::thread` menjalankan batch. Hasil dikirim balik lewat `async_channel` dan
  diproses di main context (`glib::spawn_future_local`) supaya tidak ada mutasi UI dari
  thread lain. Satu konversi pada satu waktu (`converting` flag).
- Output ditulis di direktori sumber; nama yang sudah ada tidak ditimpa (error dilaporkan).
- Dismiss: `Escape` dan klik di luar `panel.compute_bounds()` dikirim sebagai
  `ActionRequest::Dismiss` ke controller; klik di dalam wedge tidak menutup panel.
- Panel selalu memakai kelas `.rd-panel` dan palet `action_wheel::palette()`; roda folder
  memakai komponen/ warna yang sama agar light/dark konsisten.

## Roda folder proyek

- `launcher.rs` membuat layer-shell overlay dengan ukuran sama seperti roda konversi;
  `wheel.rs` menggambar dan melakukan hit-test.
- `WheelData` membawa project aktif, semua project (jumlah + warna untuk dot), folder
  terfilter, query, dan total project. Filter dihitung di `deck-core::search` (fuzzy).
- Layout adaptif:
  - 0 folder → empty state (cara menambah folder).
  - 1 folder → satu kartu besar, judul project di atas, badge nomor, alasan bila folder
    hilang/unmounted. Panel dipadatkan menjadi tinggi 360 px agar tidak ada ruang kosong.
  - ≥2 folder → roda penuh; label dipotong elips dan diletakkan di dalam wedge.
- Warna: project dan folder (opsional) memakai `#RRGGBB`; warna folder menimpa warna
  project pada wedge/kartu. Parsing warna aman (fallback ke default).
- Interaksi: klik = buka, klik tengah = salin path, klik kanan = menu konteks
  (buka/terminal/salin/edit/hapus), `Ctrl+Enter` = terminal, `1`–`9` = item ke-N,
  `Tab`/`Shift+Tab` = project, panah = pilih folder, `BackSpace` = hapus query,
  `Esc` = tutup.
- `manage.rs` adalah satu-satunya tempat mutasi project/folder. Setiap perubahan memanggil
  `controller.save_config()` lalu me-refresh roda/manage agar state tidak menyimpang.
- `manage.rs` memakai `FolderRowContext` dan `ManageButtons` untuk menjaga jumlah argumen
  dan memisahkan data baris dari widget.

## Persistensi

- Path: `$XDG_CONFIG_HOME/radial-deck/config.json` (fallback `~/.config/...`).
- Tulis atomik: file sementara `.<name>.<pid>.tmp` di direktori yang sama → fsync →
  `rename` → fsync direktori. Mode `0600`.
- Load:
  - file tidak ada → config default;
  - JSON rusak → file lama dipertahankan sebagai `config.json.corrupt`, app mulai
    dengan default dan memberi notifikasi (tidak menghapus data user);
  - schema lebih baru → app berhenti dengan pesan jelas.
- `Config::normalize` memperbaiki invariant (project minimal satu, id unik, path absolut,
  warna valid, project aktif valid, shortcut tidak duplikat) tanpa menyentuh file di disk.
- Schema 2 menambahkan `preferences.projects_hotkey`. Migrasi aditif dari schema 1; file
  dengan schema lebih baru ditolak sebelum ditulis.
- Config disimpan ulang hanya saat ada perubahan; refresh UI tidak menulis disk.

## Keamanan

- Tidak ada shell. `xdg-open`, terminal, dan seluruh konverter dipanggil dengan `Command`
  + argv; nama executable konverter berasal dari konstanta, target id berasal dari registry
  tetap, dan path file menjadi argumen terpisah (tanpa escaping shell).
- Path sumber divalidasi ada dan dinormalisasi; output selalu di direktori sumber dan
  operasi menolak menimpa file yang sudah ada.
- Format target tidak dapat diinjeksi dari CLI; hanya id yang ada di registry kategori
  yang dijalankan.
- Tidak ada evaluasi kode/konfigurasi; JSON hanya data.
- Tidak ada jaringan; notifikasi, tray, portal memakai D-Bus sesi.
- Path folder disimpan sebagai path absolut; tidak ada bookmark/token karena app native
  tidak memakai sandbox. Versi Flatpak memerlukan portal dokumen/file chooser.
- Dependensi di-pin lewat `Cargo.lock`; CI memakai `--locked`.

## Performa

- Event-driven: tidak ada polling. Hotkey/tray menunggu sinyal D-Bus; UI hanya redraw saat
  interaksi.
- Overlay dibuat lazy saat pertama dipakai; idle hanya tray + hotkey + GTK loop.
- Konversi berjalan di thread terpisah; UI tidak memblokir dan progres tidak memicu polling.
- `GSK_RENDERER=cairo` dipaksa sebelum GTK init (kecuali user meng-override). Ini
  mengurangi RSS window pertama dari ~115 MB ke ~68 MB dan tidak membutuhkan GPU.
- Wheel digambar dengan Cairo (CPU) hanya saat tampil/berubah.
- Window manage/settings dihancurkan saat ditutup, bukan disembunyikan, agar widget tree
  dilepas. Overlay roda tetap dipakai ulang setelah dismiss.
- Release idle (hidden, setelah startup): ~53 MB RSS. Setelah window pertama: ~68–87 MB
  RSS. Binary 6.5 MiB (stripped). Diukur pada 2026-10-23, KDE Wayland.
- Tidak ada polling; hotkey/tray menunggu D-Bus, UI hanya redraw saat interaksi.

## Struktur kode Linux

```text
crates/deck-core/src/
├── convert/registry.rs  # kategori, target, availability, deteksi tool
├── convert/image.rs     # native image crate + ImageMagick fallback
├── convert/external.rs  # ffmpeg, pdftoppm/pdftotext, LibreOffice, pandoc
├── convert/archive.rs   # ZIP/TAR/GZIP (compress + extract)
└── convert/mod.rs       # orkestrasi batch, Outcome, naming

crates/radial-deck/src/
├── main.rs              # GtkApplication, single instance, CLI
├── controller.rs        # state + semua mutasi + jembatan UI + worker konversi
├── action_launcher.rs   # overlay panel konversi, tab, drag & drop, dismiss
├── action_wheel.rs      # roda konversi Cairo, hit-test, keyboard, hover
├── launcher.rs          # layer-shell overlay roda folder
├── wheel.rs             # roda folder: drawing, hit-test, context menu
├── manage.rs            # CRUD project/folder, file chooser, drag & drop
├── settings.rs          # shortcut, autostart, terminal, tema, path log
├── hotkey.rs            # GlobalShortcuts portal, dua binding (tokio)
├── logging.rs           # log harian di XDG_STATE_HOME, retensi 7 file
├── tray.rs              # StatusNotifierItem (ksni)
├── actions.rs           # xdg-open, terminal, clipboard
├── autostart.rs         # XDG autostart desktop entry
└── theme.rs             # CSS + deteksi dark (GTK + KDE color scheme)
```

## Verifikasi

- Unit test `deck-core`: validasi folder, normalisasi config, ranking search, persistensi
  atomik + permission, schema, availability (missing vs unmounted).
- CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --locked`, build Arch.
- Uji manual Wayland: hotkey portal, overlay layer-shell, fokus keyboard, launch folder,
  tray, restart persistence.
- Uji end-to-end konversi (KDE Wayland, klik roda): PNG→JPG, WAV→MP3, PNG→WEBP;
  verifikasi file dengan `file`/`ffprobe`; single-instance forwarding; dismiss Esc/klik luar.
