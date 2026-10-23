# Arsitektur

## Gambaran

```text
┌──────────────────────────── radial-deck (GTK4, main thread) ───────────────────────────┐
│ action_launcher.rs (panel, tabs, DnD) + action_wheel.rs (Cairo)                         │
│ launcher.rs + wheel.rs (roda folder)      manage.rs      settings.rs                   │
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

1. `hotkey.rs` membuat sesi `org.freedesktop.portal.GlobalShortcuts` dan meminta binding
   `toggle` dengan trigger pilihan (`Ctrl+Alt+Space` default).
2. Portal (KDE) menyimpan persetujuan user; sinyal `Activated` diteruskan sebagai
   `HotkeyEvent::Activated`.
3. Controller men-toggle overlay. Binding tidak memakai `XGrabKey`/Carbon sehingga aman
   di Wayland dan tidak mencuri input saat aplikasi lain aktif.
4. `ConfigureShortcuts` membuka UI portal; setelah selesai, binding dibaca ulang dan
   ditampilkan di Settings/tray.

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
- Dismiss: `Escape` di level window (capture phase) dan klik pada scrim (pick-test),
  sehingga klik di wedge tidak ikut menutup panel.

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
  warna valid, project aktif valid) tanpa menyentuh file di disk.

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
- Tidak menyentuh GPU NVIDIA; GTK/KWin memakai renderer default Intel.
- Wheel digambar dengan Cairo (CPU) hanya saat tampil/berubah.
- Release idle: ~25 MB PSS (~48 MB RSS termasuk shared library GTK), binary stripped 6.3 MB (diukur pada rilis 0.1.0).

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
├── settings.rs          # hotkey portal, autostart, terminal, tema
├── hotkey.rs            # GlobalShortcuts portal (tokio)
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
