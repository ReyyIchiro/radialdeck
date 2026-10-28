# Arsitektur

## Gambaran

```text
┌──────────────────────────── radial-deck (GTK4, main thread) ───────────────────────────┐
│ action_launcher.rs + action_wheel.rs (konversi, Cairo)                                 │
│ launcher.rs + project_tabs.rs + wheel.rs         manage.rs   settings.rs                │
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
   default `Ctrl+Alt+Q`).
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
- Wayland tidak mengizinkan query posisi pointer global. Yang tersedia adalah
  `gdk_display_get_monitor_at_surface(pointer surface)`, jadi `surface.rs` memilih monitor
  yang sedang ditunjuk pointer lalu memanggil `set_monitor` sebelum surface tampil; wheel
  berada di tengah monitor itu, bukan di titik kursor. Tanpa ini compositor memilih output
  default, sehingga pada dua monitor roda bisa muncul di layar yang salah.
- `gtk4_layer_shell::is_supported()` bernilai true juga di X11 (library terpasang), padahal
  `init_layer_shell` di sana hanya menghasilkan GTK critical dan surface tidak pernah
  di-map. `surface::layer_shell_available()` karena itu memeriksa nama display Wayland
  lebih dulu. Di X11 aplikasi memakai jendela biasa yang di-`fullscreen_on_monitor` pada
  monitor pointer; posisi kursor tetap tidak dipakai karena GTK4 tidak menyediakan move
  window lintas backend — dicatat sebagai keterbatasan.
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

## Mode roda: Convert, Tools, Actions

- Satu `ActionLauncher` merender tiga mode; `WheelMode` menentukan isi wedge, caption, dan
  aksi klik. `Shift` → Convert, `Super` → Tools, `Ctrl` → Actions; `F1`–`F3` adalah
  alternatif agar tetap bisa dipakai pada keyboard tanpa modifier tersebut.
- Mode hotkey hanya aktif saat tidak ada form terbuka. Form mengklaim keyboard: `Ctrl+A`
  harus memilih teks, bukan berpindah ke Actions. Sebelum perbaikan ini `Ctrl+A` saat
  mengedit kualitas melempar pengguna ke roda Actions dan menjalankan Float.
- `Enter`/`Esc` di dalam form dikirim sebagai `ToolRun`/`ToolCancel`; `Esc` tanpa form
  mengirim `Dismiss`.
- Mode Tools memakai `ToolId::available_for(category)`; mode Actions memakai daftar tetap
  dengan `reason` per item (mis. **Send** redup tanpa perangkat KDE Connect).

## File tools

- `deck-core::tools` adalah lapisan domain tanpa UI: `ToolId`, `Field` (key, label, nilai
  default, hint, pilihan), `availability`, `ToolRequest`, `ToolOutcome`.
- Eksekusi memakai binary eksternal (`magick`/`convert`, `ffmpeg`/`ffprobe`, `gs`,
  `pdfseparate`/`pdfunite`, `7z`/`7zz`) yang dipanggil via `Command` tanpa shell. Tidak ada
  string dari input pengguna yang masuk ke command line sebagai argumen mentah: nilai
  numerik di-parse, warna divalidasi `#rrggbb`, dan nama file selalu argumen terpisah.
- Setiap tool menulis ke `unique_path`/`unique_dir` di folder sumber sehingga file yang
  sudah ada tidak pernah ditimpa; kegagalan per file dikumpulkan di `ToolOutcome::failures`.
- `ToolOutcome::merge` menggabungkan hasil beberapa tool/batch; `created` dipakai untuk
  notifikasi dan pemilihan file baru.

## Aksi file dan Float

- `actions.rs` memegang operasi yang menyentuh desktop: buka folder (`xdg-open`), salin path,
  salin file sebagai `text/uri-list`, terminal, KDE Connect, dan validasi nama executable.
  Terminal dipilih dari tabel tetap dengan flag per terminal, sehingga preferensi pengguna
  tidak bisa menyelundupkan argumen.
- `float_shelf.rs` membuat jendela 340×420 tanpa dekorasi. Di Wayland ia layer-shell
  `Overlay` + `KeyboardMode::OnDemand` yang di-anchor ke kanan atas monitor pointer; di X11
  menjadi jendela biasa. Tiap baris adalah `DragSource` yang menyediakan `text/uri-list`,
  jadi file bisa ditarik kembali ke file manager.
- Shelf dipakai ulang antar pemanggilan; bila window destroyed (compositor restart),
  controller membuat ulang saat **Float** dipilih lagi.

## Roda folder proyek

- `launcher.rs` membuat layer-shell overlay; `project_tabs.rs` merender rail dan
  `wheel.rs` menggambar serta melakukan hit-test.
- `WheelData` membawa project aktif, semua project (untuk dot warna), folder terfilter,
  query, dan flag fokus rail. Filter dihitung di `deck-core::search` (fuzzy).
- Layout adaptif:
  - 0 folder / tidak ada hasil → empty state dengan CTA **Manage folders**.
  - 1 folder → satu target cincin penuh; pusat membuka folder, badge alasan tampil bila
    folder hilang/unmounted. Tidak ada sektor kosong.
  - ≥2 folder → roda penuh; label ditempatkan di tengah ring, dengan font lebih kecil dan
    lebar dibatasi untuk label samping agar tidak menabrak pusat atau tepi luar.
- `project_tabs.rs` merender maksimal lima tab. Project aktif selalu terlihat; sisanya
  masuk menu `+N`. Tab aktif memakai border oranye, fokus keyboard memakai border putih.
  Tab dari wheel ditangkap di phase capture agar fokus berpindah sebelum GTK traversal.
- Warna: project dan folder (opsional) memakai `#RRGGBB`; warna folder menimpa warna
  project pada wedge. Parsing warna aman (fallback ke default).
- Interaksi: klik tab = ganti project, klik wedge = buka, klik tengah wedge = salin path,
  klik kanan = menu konteks (buka/terminal/salin/edit/hapus), `Ctrl+Enter` = terminal,
  `1`–`9` = item ke-N, panah = pilih folder, `Tab` = pindah fokus ke rail (lalu `Enter`),
  `BackSpace` = hapus query, `Esc` = tutup. Pusat = Manage, kecuali hanya ada satu folder
  (buka folder) atau query aktif (hapus query).
- `manage.rs` adalah satu-satunya tempat mutasi project/folder. Setiap perubahan memanggil
  `controller.save_config()` lalu me-refresh roda/manage agar state tidak menyimpang.
- `manage.rs` memakai `FolderRowContext` untuk memisahkan data baris dari widget, serta
  rail project yang sama dengan roda supaya urutan/warna konsisten.

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
- Schema 2 menambahkan `preferences.projects_hotkey`. Migrasi aditif dari schema 1.
- Schema 3 memindahkan default shortcut project dari `CTRL+ALT+P` ke `CTRL+ALT+Q`; hanya
  nilai default lama yang dimigrasikan, shortcut pilihan pengguna dibiarkan. File dengan
  schema lebih baru ditolak sebelum ditulis.
- Schema 4 menambahkan riwayat operasi; schema 5 menambahkan `preferences.output_location`,
  `preferences.output_subfolder`, dan `preferences.wheel_tools`. Keduanya aditif: config
  lama tetap terbaca dan mendapat nilai default saat di-normalisasi.
- `wheel_tools` memakai `Option<&[String]>` di API: kunci yang tidak ada berarti "pakai
  default", sedangkan daftar kosong berarti "sembunyikan semua tool" — pilihan eksplisit
  pengguna tidak boleh diam-diam kembali ke default.
- Config disimpan ulang hanya saat ada perubahan; refresh UI tidak menulis disk.

## Keamanan

- Tidak ada shell. `xdg-open`, terminal, seluruh konverter, dan seluruh file tool dipanggil
  dengan `Command` + argv; nama executable berasal dari konstanta, target/tool id berasal
  dari registry tetap, dan path file menjadi argumen terpisah (tanpa escaping shell).
- Nilai form divalidasi sebelum masuk argv: angka di-parse, rasio/persentase dihitung
  terhadap dimensi sumber, warna harus `#rrggbb`, dan pilihan berasal dari daftar `Field`.
  Nama terminal dari settings harus nama executable polos (tanpa `/`, spasi, atau flag).
- Device id KDE Connect divalidasi sebagai hex sebelum dikirim ke `kdeconnect-cli`.
- Path sumber divalidasi ada dan dinormalisasi; operasi menolak menimpa file yang sudah
  ada dan memakai `unique_path` untuk memberi sufiks angka.
- Lokasi output dibatasi ke direktori sumber, subfolder di dalamnya, atau folder absolut
  pilihan pengguna; nama subfolder tidak boleh mengandung `..` (dinormalisasi menjadi
  `.._`), sehingga tidak bisa keluar dari folder sumber.
- Setiap path output didaftarkan di scope operasi sebelum ditulis. Saat Cancel ditekan,
  proses anak dibunuh lalu hanya path yang terdaftar yang dihapus, sehingga file milik
  pengguna tidak pernah tersentuh.
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
├── convert/mod.rs       # orkestrasi batch, Outcome, naming
├── tools/mod.rs         # ToolId, Field, availability, execute, ToolOutcome
├── tools/image.rs       # compress, crop, edit, annotate, frame, redact, collage
├── tools/media.rs       # video/audio: trim, crop, split, speed, snapshot, join, mute
├── tools/pdf.rs         # create/merge/split/organize, extract archive
├── tools/shell.rs       # deteksi binary, eksekusi, unique path, parsing nilai
└── file_actions.rs      # copy/move/trash tanpa shell (dipakai mode Actions)

crates/radial-deck/src/
├── main.rs              # GtkApplication, single instance, CLI
├── controller.rs        # state + semua mutasi + jembatan UI + worker konversi
├── action_launcher.rs   # overlay panel konversi, tab, mode, form, drag & drop
├── action_wheel.rs      # roda konversi Cairo, hit-test, keyboard, hover
├── actions.rs           # aksi desktop: clipboard, terminal, KDE Connect, validasi
├── float_shelf.rs       # jendela Float, baris drag source
├── surface.rs           # deteksi layer-shell + penempatan monitor pointer
├── launcher.rs          # layer-shell overlay roda folder + rail project
├── wheel.rs             # roda folder: drawing, hit-test, context menu
├── project_tabs.rs      # tab project, overflow +N, sinkronisasi state
├── widgets.rs           # tombol ikon, klaster warna, helper aksesibilitas
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
