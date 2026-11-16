# RadialDeck Manual

Dokumen referensi lengkap untuk pengguna dan agen AI. README adalah ringkasan;
dokumen ini menjelaskan semua yang bisa dilakukan aplikasi, satu halaman saja.
Sumber kebenaran untuk status verifikasi: [`status.md`](status.md). Batas modul:
[`architecture.md`](architecture.md). Versi dokumen mengikuti versi aplikasi di
`Cargo.toml` (saat ini 0.3.0, schema config 5).

---

## 1. Gambaran cepat

RadialDeck adalah overlay radial untuk Linux. Dua alur terpisah:

| Alur | Hotkey default | Isi |
|---|---|---|
| Quick actions | `Ctrl+Alt+Space` | Convert (konversi), Tools (26 tool), Actions (aksi file) |
| Project folders | `Ctrl+Alt+Q` | Roda folder proyek per project |

Aplikasi berjalan sebagai daemon tray. Idle ~53 MB RSS (renderer Cairo), tanpa
GPU, tanpa polling. Sumber file bisa datang dari: argumen CLI, drag & drop ke
panel, klik kanan Dolphin (service menu), atau dibuka kosong lalu diisi.

## 2. CLI lengkap

| Perintah | Efek |
|---|---|
| `radial-deck FILE...` | Buka roda konversi untuk file |
| `radial-deck --convert FILE...` | Paksa mode Convert |
| `radial-deck --tools FILE...` | Langsung mode Tools |
| `radial-deck --actions FILE...` | Langsung mode Actions |
| `radial-deck --show` | Roda konversi kosong |
| `radial-deck --projects` | Roda folder proyek |
| `radial-deck --hidden` | Start di tray (dipakai autostart) |
| `radial-deck --manage` | Jendela manajemen folder |
| `radial-deck --settings` | Jendela settings (6 halaman) |
| `radial-deck --reset-config` | Hapus config, mulai default |
| `radial-deck --version` / `--help` | Versi / bantuan |
| `radial-deck convert --to FMT FILES...` | Konversi headless (tanpa GTK, aman di SSH) |
| `radial-deck convert --into DIR ...` | Semua output ke satu folder |
| `radial-deck convert --subfolder NAME ...` | Output ke `<folder sumber>/<NAME>` |
| `radial-deck convert --list [FILES...]` | Daftar target yang tersedia (per kategori file, atau semua kategori) |
| `radial-deck convert --quiet` | Cetak error saja |

Exit code batch: `0` semua sukses, `1` ada kegagalan, `2` argumen/target salah.
Path hasil dicetak satu per baris; nama bentrok dapat sufiks `-1`, `-2`, … dan
file yang ada tidak pernah ditimpa.

Contoh:

```bash
radial-deck convert --to webp foto1.png foto2.png
radial-deck convert --to mp4 --subfolder Converted clip.mov
radial-deck convert --list            # overview semua kategori
radial-deck convert --list film.mkv   # target yang valid untuk file ini
```

## 3. Interaksi roda quick actions

| Input | Aksi |
|---|---|
| `Shift` / `Super` / `Ctrl` | Ganti mode Convert / Tools / Actions (live) |
| `F1` `F2` `F3` | Alternatif ganti mode tanpa modifier |
| Panah / scroll | Pilih wedge |
| `1`..`9` | Jalankan item ke-N |
| `Enter` | Terapkan / jalankan |
| `Esc` | Tutup form dulu bila terbuka, lalu tutup roda |
| Klik area gelap | Tutup roda |
| Drag file ke panel | Ganti sumber; kategori mengikuti file pertama |

### 3.1 Convert

Sepuluh kategori: Images, Video, Audio, PDF, Documents, Slides, Spreadsheets,
EPUB, Subtitles, Archives. Target yang tidak bisa dijalankan tampil redup
dengan alasan (tool belum terpasang, format tidak cocok). Arsip ZIP/TAR/GZIP
ditangani Rust native; 7Z/RAR perlu `7zip`.

### 3.2 Tools (26)

Compress, Crop, Edit (brightness/contrast/saturation/rotate/flip), Annotate,
Frame, Redact, Create PDF, Merge PDFs, Split PDF, Organize PDF, Join Videos,
Trim, Crop Video, Change Speed, Snapshots, Split Video, Mute, Normalize Audio,
Channels, Audio to Video, Extract Archive, dan lainnya. Tool berparameter
membuka form; `Enter` menjalankan, `Esc` kembali. Saat form terbuka, modifier
tetap berfungsi sebagai teks (`Ctrl+A` memilih teks, bukan ganti mode).

Urutan dan visibilitas tool per kategori diatur di Settings, Tool wheel; urutan
menentukan digit `1`..`9`.

### 3.3 Actions

Copy, Move, **To Project**, Copy Path, Terminal, Send (KDE Connect), Float
(shelf mengambang 340x420, tiap baris drag source). Probe KDE Connect berjalan
di thread terpisah; roda dirender dulu, target Send di-patch saat jawaban tiba.

**To Project** menghubungkan dua roda: roda folder terbuka dalam mode picker
(banner `Pick a folder to move N file(s) · Esc cancels`); folder yang dipilih
menjadi tujuan pemindahan. Esc, klik luar, atau klik pusat membatalkan.

### 3.4 Panel progres

Muncul pojok kanan bawah (layer-shell, tanpa fokus keyboard): judul operasi,
penghitung `done / total`, badge merah `N failed`, fase manusia plus persentase
per file yang jujur bila FFmpeg ikut menyertakan `Duration:` dan `time=`
(`foto.mp4 · 64% · ~18s left`), bar aksen, tombol Cancel. Cancel menghentikan
child dan menghapus output parsial yang terdaftar. Ringkasan masuk notifikasi
desktop; hasil masuk riwayat tray (Recent Operations, Undo hanya menghapus file
yang dibuat operasi itu).

## 4. Roda folder proyek

Rail atas menampung maksimal 5 project; sisanya di menu `+N`. Filter real-time
memakai ranking bersama: label diawali query, lalu label mengandung, lalu path
mengandung. Satu folder = cincin penuh. `Tab` pindah fokus ke rail; `Ctrl+Enter`
buka terminal; klik tengah wedge salin path; klik kanan menu konteks. Pusat =
Manage (atau buka folder saat hanya satu).

## 5. Settings (6 halaman)

| Halaman | Isi |
|---|---|
| General | Tema (ikuti sistem/terang/gelap), live-modifier on/off |
| Shortcuts | Dua shortcut via portal, tombol Change Shortcuts |
| Projects | Daftar folder per project, pintasan ke Manage |
| Tool wheel | Urutan + visibilitas tool per kategori |
| Output | Lokasi output: alongside / subfolder / folder tetap |
| Advanced | Path config + log, tombol buka keduanya, status dependensi + Recheck |

Tema: mengikuti color scheme KDE + GTK; paksa Light/Dark tersedia. Aksen
mengikuti **AccentColor** KDE bila diatur (dibaca sekali saat start), dengan
kontras yang dihitung otomatis; fallback oranye `#ff5a00`. Recheck
menyegarkan cache tool sehingga program yang baru dipasang terdeteksi tanpa
restart.

## 6. Konfigurasi dan data

| Path | Isi |
|---|---|
| `$XDG_CONFIG_HOME/radial-deck/config.json` | Satu-satunya config (schema 5, mode 0600, tulis atomik) |
| `$XDG_STATE_HOME/radial-deck/history.json` | Riwayat operasi (maks 30 entri, 0600, atomik) |
| `$XDG_STATE_HOME/radial-deck/logs/` | Log harian, retensi 7 file |

Config tidak perlu diedit manual. Schema lebih baru dari build ditolak dengan
pesan jelas, tidak pernah didowngrade. JSON rusak diselamatkan sebagai
`config.json.corrupt`. Kunci preferences: `hotkey`, `projects_hotkey`,
`live_modifier`, `theme`, `output_location`, `output_subfolder`, `wheel_tools`.

## 7. Dependensi

Wajib: `gtk4`, `gtk4-layer-shell`, `xdg-desktop-portal`, `xdg-utils`.
Opsional (roda mendeteksi otomatis): `imagemagick`, `ffmpeg`, `poppler`,
`libreoffice`, `pandoc`, `ghostscript`, `7zip`, `kdeconnect`. Status semua
dependensi terlihat di Settings, Advanced (Recheck) atau
`radial-deck convert --list`.

## 8. Diagnosis

1. Log: `RUST_LOG=radial_deck=debug radial-deck --show`, atau Settings,
   Advanced, Open Logs. Panic tercatat di log harian.
2. Hotkey mati: `./scripts/doctor.sh` (laporan) atau `--fix` (restart portal).
   Detail penyebab portal KDE ada di README, Troubleshooting.
3. Uji tanpa menyentuh layar: `scripts/ui-smoke.sh` (45 cek, sesi KWin virtual).

## 9. Peta kode (untuk agen AI)

| Crate | Peran |
|---|---|
| `deck-core` | Domain tanpa UI: config, store atomik, search ranking, registry konversi, 26 tool, job scope (progres/cancel/cleanup), history, `binaries` (satu sumber nama biner). Tidak boleh depend GTK/Wayland. |
| `radial-deck` | Shell GTK4: `controller.rs` satu-satunya pemilik state; `action_*`/`wheel.rs`/`launcher.rs` roda; `hotkey.rs` portal (tokio current-thread); `tray.rs` ksni; `batch.rs` CLI headless. |

Invarian yang tidak boleh dilanggar (detail di `AGENTS.md`): satu overlay
terlihat; dismiss hanya lewat controller; layer-shell Overlay + Exclusive +
zone -1 hanya untuk dua roda; mutasi widget hanya di main context; config tulis
atomik 0600; tanpa shell (semua proses via argv); dua ID portal tetap.

Aturan kerja: selesai = `cargo fmt` + `clippy -D warnings` + `cargo test` +
`cargo build --release --locked` + `scripts/ui-smoke.sh` untuk perubahan UI
(termasuk golden: tiga render roda dibandingkan `scripts/golden`; regen
`RADIAL_DECK_GOLDEN_REGEN=1` setelah perubahan visual disengaja) + satu
konversi nyata untuk perubahan konversi + `scripts/bench.sh` bila jalur
panas tersentuh. Perbarui README, `status.md`, dan bila batas modul berubah
`architecture.md`.
