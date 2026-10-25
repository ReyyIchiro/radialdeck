# Panduan Kerja Proyek (AGENTS)

Dokumen ini adalah titik masuk untuk kontributor manusia maupun agen AI. Baca bersama
[`docs/status.md`](docs/status.md) (kondisi terverifikasi) dan
[`docs/architecture.md`](docs/architecture.md) (batas modul).

## Tujuan dan batasan

- Aplikasi produktivitas Linux: roda aksi file (konversi/arsip) dan roda folder proyek.
- Target utama: CachyOS/Arch, KDE Plasma 6, Wayland. X11 adalah fallback.
- Ringan: tanpa polling, tanpa GPU NVIDIA, startup cepat.
- Bahasa komunikasi dan dokumentasi: Indonesia. Komentar kode dan identifier: Inggris.
- Jangan menyalin kode dari proyek referensi. Implementasi ini MIT dan orisinal.

## Peta repositori

| Path | Isi |
|---|---|
| `crates/deck-core/` | Logika tanpa UI: config/folder, store XDG atomik, pencarian, registry + eksekusi konversi |
| `crates/radial-deck/` | Shell Linux GTK4: overlay layer-shell, roda Cairo, manage, settings, hotkey portal, tray |
| `docs/` | Riset, arsitektur, status hidup, roadmap |
| `packaging/` | Desktop entry, service menu Dolphin, PKGBUILD/AUR, Flatpak (WIP) |
| `scripts/` | `install.sh`, `uninstall.sh` user-local, `ui-smoke.sh` (sesi KWin virtual) |

`deck-core` tidak boleh bergantung pada GTK, Wayland, portal, atau path Linux. Shell
platform baru harus memakai crate core ini, bukan menduplikasi model/persistensi/konversi.

## Perintah wajib sebelum selesai

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --locked
```

Perubahan UI diuji lebih dulu dengan `scripts/ui-smoke.sh` (sesi KWin virtual: layar dan
pointer pengguna tidak tersentuh). Setelah itu uji di sesi nyata (KDE Wayland) minimal:
buka/tutup overlay lewat hotkey, klik luar, `Esc`, manage, settings, dan launch folder.
Jangan mengklaim UI berhasil hanya dari kompilasi. Untuk perubahan konversi, jalankan satu
konversi nyata dan periksa file hasil.

## Invariant yang mudah rusak

1. **Satu overlay terlihat.** Membuka roda folder harus menutup roda aksi, dan sebaliknya.
   Manage/settings menutup keduanya.
2. **Dismiss hanya lewat controller.** Jangan memanggil `set_visible(false)` langsung dari
   handler klik/key di dalam widget overlay. State controller (`visible` flag) harus selalu
   sama dengan state window; jika tidak, hotkey berikutnya hanya menutup window tak terlihat
   (bug lama).
3. **Layer-shell:** overlay memakai `Layer::Overlay`, `KeyboardMode::Exclusive`,
   `exclusive_zone=-1`. Dua surface exclusive tidak boleh aktif bersamaan.
4. **Main thread GTK.** Mutasi widget hanya dari main context. Worker konversi mengirim
   hasil lewat `async_channel` + `glib::spawn_future_local`.
5. **Config:** tulis atomik (temp → fsync → rename), mode `0600`, path XDG. Migrasi schema
   harus kompatibel ke belakang; schema yang lebih baru ditolak dengan pesan jelas.
6. **Keamanan:** tanpa shell; semua proses memakai `Command` + argv. Path divalidasi absolut
   dan ada. Jangan menambah eksekusi string dari config/input.
7. **Hotkey:** dua ID portal tetap: `toggle` (aksi cepat) dan `projects` (folder). Event
   portal harus diperbarui per-ID; jangan mengasumsikan urutan atau satu binding saja.

## Alur UI saat ini

- `controller.rs` adalah satu-satunya pemilik state aplikasi dan mutasi config.
- `action_launcher.rs` + `action_wheel.rs`: roda konversi.
- `launcher.rs` + `wheel.rs`: roda folder. Keduanya memakai panel `.rd-panel` dan palet
  `action_wheel::palette()` agar light/dark konsisten.
- `manage.rs`: CRUD project/folder, rename, warna, reorder, buka/terminal/salin path.
- `settings.rs`: dua shortcut, autostart, terminal, tema, path config/log.
- `hotkey.rs`: sesi portal + dua shortcut; `tray.rs`: menu/tooltip dengan dua shortcut.

## Log dan diagnosis

- Runtime log: `$XDG_STATE_HOME/radial-deck/logs/radial-deck.YYYY-MM-DD.log`
  (fallback `~/.local/state/...`), retensi 7 file harian.
- Debug: `RUST_LOG=radial_deck=debug radial-deck --show`.
- Log juga tersedia lewat **Settings → Diagnostics → Open Logs**.
- Sertakan log saat melaporkan bug overlay/hotkey/konversi.

## Alur dokumen

Setiap perubahan perilaku wajib memperbarui:

1. `README.md`: fitur, shortcut, CLI, status verifikasi.
2. `docs/status.md`: tanggal, hasil tes, known issues, antrean berikutnya.
3. `docs/architecture.md` bila batas modul/alur berubah.
4. `docs/roadmap.md` bila prioritas atau keputusan platform berubah.

Gunakan commit kecil dengan Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`). Jangan mencampur refactor besar dengan perubahan perilaku tanpa alasan.
