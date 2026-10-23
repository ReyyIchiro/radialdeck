# Arsitektur

## Gambaran

```text
┌──────────────────────────── radial-deck (GTK4, main thread) ───────────────────────────┐
│ launcher.rs  overlay layer-shell + wheel.rs (Cairo)   manage.rs   settings.rs           │
│        ▲                     ▲                              ▲            ▲              │
│        │                     │                              │            │              │
│                     controller.rs (state, persistence, actions, windows)               │
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
│ deck-core: Config, Project, Folder, availability, search, Store (XDG, atomic save)      │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

## Workspace

| Crate | Tanggung jawab | Dependensi UI/OS |
|---|---|---|
| `crates/deck-core` | model, validasi, pencarian, persistensi atomik, availability | tidak ada |
| `crates/radial-deck` | shell Linux: GTK4, layer-shell, portal, tray, autostart, aksi | ya (Linux) |

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

- Tidak ada shell. `xdg-open`/terminal dipanggil dengan `Command` + argv; input user hanya
  boleh berupa nama executable polos untuk terminal (tanpa `/`, spasi, newline), argumen
  dipilih dari tabel tetap.
- Tidak ada evaluasi kode/konfigurasi; JSON hanya data.
- Tidak ada jaringan; notifikasi, tray, portal memakai D-Bus sesi.
- Path folder disimpan sebagai path absolut; tidak ada bookmark/token karena app native
  tidak memakai sandbox. Versi Flatpak memerlukan portal dokumen/file chooser.
- Dependensi di-pin lewat `Cargo.lock`; CI memakai `--locked`.

## Performa

- Event-driven: tidak ada polling. Hotkey/tray menunggu sinyal D-Bus; UI hanya redraw saat
  interaksi.
- Overlay dibuat lazy saat pertama dipakai, sehingga idle hanya tray + hotkey + GTK loop.
- Tidak menyentuh GPU NVIDIA; GTK/KWin memakai renderer default Intel.
- Wheel digambar dengan Cairo (CPU) hanya saat tampil/berubah.

## Struktur kode Linux

```text
crates/radial-deck/src/
├── main.rs          # GtkApplication, single instance, CLI
├── controller.rs    # state + semua mutasi + jembatan UI
├── launcher.rs      # layer-shell overlay window
├── wheel.rs         # radial drawing, hit-testing, keyboard, context menu
├── manage.rs        # CRUD project/folder, file chooser, drag & drop
├── settings.rs      # hotkey portal, autostart, terminal, tema
├── hotkey.rs        # GlobalShortcuts portal (tokio)
├── tray.rs          # StatusNotifierItem (ksni)
├── actions.rs       # xdg-open, terminal, clipboard
├── autostart.rs     # XDG autostart desktop entry
└── theme.rs         # CSS + deteksi dark
```

## Verifikasi

- Unit test `deck-core`: validasi folder, normalisasi config, ranking search, persistensi
  atomik + permission, schema, availability (missing vs unmounted).
- CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --locked`, build Arch.
- Uji manual Wayland: hotkey portal, overlay layer-shell, fokus keyboard, launch folder,
  tray, restart persistence.
