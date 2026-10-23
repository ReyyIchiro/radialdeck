# RadialDeck

Overlay launcher untuk folder proyek di Linux (Wayland/X11, KDE Plasma).

Buka folder kerja dari mana saja dengan satu shortcut keyboard: roda jari-jari muncul
di tengah layar, ketik untuk filter, klik atau tekan angka untuk buka.

> Terinspirasi oleh [Tangerine for Mac](https://tangerineformac.com/) dan
> [ProjectDeck](https://github.com/nizar268/Shortcut-Floating).
> Tidak ada kode yang disalin; implementasi asli, lisensi MIT.

---

## Fitur

| Fitur | Status |
|---|---|
| Overlay roda radial (Wayland layer-shell) | ✅ |
| Shortcut global via XDG GlobalShortcuts portal | ✅ |
| Pencarian real-time (ketik langsung) | ✅ |
| Navigasi keyboard (angka, panah, Tab, Enter, Esc) | ✅ |
| Klik kiri buka · klik kanan menu konteks | ✅ |
| Multi-project dengan Tab/← → | ✅ |
| Manage folder (drag-drop, file chooser) | ✅ |
| System tray (StatusNotifierItem) | ✅ |
| Tema ikut sistem / paksa Light / Dark | ✅ |
| Autostart XDG | ✅ |
| Buka terminal di folder (Ctrl+Enter) | ✅ |
| Salin path ke clipboard (klik tengah) | ✅ |
| Konfigurasi JSON atomik di XDG_CONFIG_HOME | ✅ |

---

## Kebutuhan Sistem

- **OS:** Linux (Arch/CachyOS diuji; Debian/Ubuntu kompatibel)
- **Desktop:** KDE Plasma 6 (Wayland, primary target) atau X11 fallback
- **Runtime:** gtk4, gtk4-layer-shell, xdg-desktop-portal, xdg-utils

---

## Instalasi

### Cara cepat (user-local, tanpa sudo)

```bash
git clone https://github.com/raihan/radial-deck.git
cd radial-deck
./scripts/install.sh
```

Script akan build release binary, install ke `~/.local/bin`, dan daftarkan desktop
entry agar portal GlobalShortcuts bisa mengenali aplikasi.

### Arch / CachyOS (AUR)

```bash
cd packaging/aur
makepkg -si
```

Atau gunakan AUR helper (setelah versi pertama di-publish):

```bash
paru -S radial-deck
```

### Build manual

```bash
# install deps (Arch)
sudo pacman -S gtk4 gtk4-layer-shell xdg-desktop-portal rust

cargo build --release --locked
install -Dm755 target/release/radial-deck ~/.local/bin/radial-deck
```

---

## Penggunaan

| Perintah | Aksi |
|---|---|
| `radial-deck --hidden` | Mulai tersembunyi di tray (untuk autostart) |
| `radial-deck --show` | Langsung tampilkan overlay |
| `radial-deck --manage` | Buka jendela manajemen folder |
| `radial-deck --settings` | Buka pengaturan |
| `radial-deck --reset-config` | Hapus konfigurasi dan mulai ulang |

### Shortcut default

| Tombol | Aksi |
|---|---|
| `Ctrl+Alt+Space` | Toggle overlay (global, via portal) |
| Ketik huruf | Filter folder |
| `1`–`9` | Buka folder ke-N |
| `Enter` | Buka folder terpilih |
| `Ctrl+Enter` | Buka terminal di folder |
| `Tab` / `← →` | Ganti project |
| `Esc` | Tutup overlay / hapus query |

### Menambah folder

1. Buka tray → **Manage Folders**, atau `radial-deck --manage`
2. Drag-drop folder ke jendela, atau klik **+ Add Folder**
3. Folder langsung tersedia di overlay

---

## Konfigurasi

File: `$XDG_CONFIG_HOME/radial-deck/config.json` (default `~/.config/radial-deck/config.json`)

Ditulis otomatis; tidak perlu edit manual. Lihat
[`crates/deck-core/src/config.rs`](crates/deck-core/src/config.rs) untuk skema.

---

## Arsitektur

Lihat [`docs/architecture.md`](docs/architecture.md) untuk diagram lengkap, model
threading, keputusan keamanan, dan rencana port Windows/macOS.

```
crates/
├── deck-core/          # model, pencarian, persistensi — tanpa UI
└── radial-deck/        # shell Linux: GTK4, layer-shell, portal, tray
    └── src/
        ├── wheel.rs    # drawing Cairo, hit-test, keyboard
        ├── manage.rs   # jendela CRUD project/folder
        ├── settings.rs # hotkey portal, autostart, tema
        ├── hotkey.rs   # GlobalShortcuts portal (tokio)
        ├── tray.rs     # StatusNotifierItem (ksni)
        └── actions.rs  # xdg-open, terminal, clipboard
```

---

## Pengembangan

```bash
# lint
cargo fmt --all && cargo clippy --all-targets -- -D warnings

# test (headless, tanpa display)
cargo test --all

# run debug
cargo run -p radial-deck -- --show
```

CI otomatis berjalan di GitHub Actions untuk setiap push dan PR ke `main`.

---

## Status Verifikasi

| Test | Hasil |
|---|---|
| Unit test (23) | ✅ semua lulus |
| Layer-shell overlay fullscreen | ✅ screenshot KDE Wayland |
| Keyboard search | ✅ filter + query real-time |
| Tab project cycling | ✅ warna center berubah |
| Config persistence | ✅ bertahan lintas proses |
| Idle RAM (release) | ~48 MB RSS |
| Binary size | 4.3 MB stripped |
| **Hotkey portal approval** | ⚠️ perlu klik OK di dialog KDE sekali |

---

## Langkah Berikutnya

- **Windows:** ganti layer-shell → Win32 topmost window; global hotkey via `RegisterHotKey`; tray via systray. Core crate tidak perlu diubah.
- **macOS:** ganti → `NSPanel`; hotkey via Carbon/AppKit; tray via `NSStatusItem`. Lihat referensi ProjectDeck.
- **Flatpak:** manifest tersedia di `packaging/flatpak/` (WIP — perlu `cargo-sources.json`).

---

## Lisensi

MIT — lihat [LICENSE](LICENSE).

Referensi: [Tangerine](https://tangerineformac.com/) (produk, UX),
[ProjectDeck](https://github.com/nizar268/Shortcut-Floating) (arsitektur macOS).
Tidak ada kode yang disalin dari keduanya.
