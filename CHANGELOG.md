# Changelog

Format mengikuti [Keep a Changelog](https://keepachangelog.com/); versi mengikuti
[Semantic Versioning](https://semver.org/). Tanggal dalam format ISO 8601.

## [0.3.0] - 2026-11-16

### Added

- `radial-deck convert --list [FILES…]`: target yang tersedia per kategori file (atau semua kategori tanpa file), satu baris per kategori, siap dipipakan ke script.
- Aksi **To Project** di roda Actions: roda folder terbuka dalam mode picker; folder yang dipilih menjadi tujuan pemindahan. Esc atau klik pusat membatalkan.
- Panel progres: penghitung `done / total`, badge `N failed`, dan persentase per file yang jujur dihitung dari `Duration:` + `time=` FFmpeg (`foto.mp4 · 64% · ~18s left`).
- Aksen aplikasi mengikuti **AccentColor** KDE (`kdeglobals`, Plasma 6) dengan kontras yang dihitung; tanpa aksen KDE dipakai oranye bawaan. Override uji: `RADIAL_DECK_FORCE_ACCENT`.
- Filter roda folder menggunakan ranking bersama `search::match_rank`: label diawali query, lalu label mengandung, lalu path mengandung.
- `deck-core::binaries`: satu sumber nama biner eksternal; requirements, registry, dan dependencies tidak lagi tersebar.
- Instalasi: PKGBUILD AUR profesional, skrip `install.sh`/`uninstall.sh` yang konsisten, dan workflow rilis GitHub (tag `v*`). Tidak ada paket konverter yang diinstal otomatis.
- Dokumentasi: `docs/installing.md`, `docs/releasing.md`, dan `docs/manual.md` diperbarui untuk pengguna dan kontributor.

### Changed

- Pengaturan Dependencies: selector paket manajer (Otomatis/Pacman/Apt/Dnf/Zypper/Lainnya), perintah instalasi yang bisa disalin per manager, badge status (Terpasang/Hilang opsional), dan panduan instalasi manual. RadialDeck tidak pernah menjalankan package manager atau `sudo`.
- Kompresi ZIP pindah ke backend zlib (`deflate-zlib`); zopfli yang membuat penulisan ZIP ~10x lebih lambat tidak lagi ada di dependency graph.
- Resolusi biner eksternal di-cache per proses; rodaroda dan konversi tidak memindai `PATH` berulang-ulang.
- Stderr proses anak sekarang benar-benar sampai ke panel progres.
- Penghitung progres batch benar; `Config::normalize` tidak pernah menurunkan schema yang lebih baru.
- `capture()` (ffprobe/identify) dapat dibatalkan dan tidak lagi memblokir tanpa batas.
- Probe KDE Connect tidak lagi berjalan di thread utama saat mode Actions dibuka.
- Panic tercatat di log harian.
- Kontras WCAG: border fokus terlihat di kedua tema, teks mode aktif ≥4.5:1, warn wedge gelap ≥4.5:1.
- `convert --list | head` dan `--help | head` tidak lagi panic karena SIGPIPE.
- Tiga render roda (Convert/Tools/Actions) diuji dengan golden-image lane di `scripts/ui-smoke.sh` (AE=0).
- PagoLayout thread-local dipakai ulang kedua roda; render terverifikasi identik piksel.
- `scripts/bench.sh`: ukuran binary, best-of-N latensi `--version` dan `convert --list`, throughput konversi; baseline tersimpan dan gagal bila regresi >25%.

## [0.2.0] - 2026-11-16

### Added

- `radial-deck convert --list [FILES…]`: target yang tersedia per kategori file
  (atau semua kategori tanpa file), satu baris per kategori, siap dipipakan ke script.
- Aksi **To Project** di roda Actions: roda folder terbuka dalam mode picker dan
  folder yang dipilih menjadi tujuan pemindahan file (dua roda terhubung jadi
  satu alur). Esc atau klik pusat membatalkan.
- Panel progres: penghitung `done / total`, badge `N failed`, dan persentase
  per file yang jujur dihitung dari `Duration:` + `time=` FFmpeg
  (`foto.mp4 · 64% · ~18s left`).
- Aksen aplikasi mengikuti **AccentColor** KDE (`kdeglobals`, Plasma 6) dengan
  kontras yang dihitung, bukan diasumsikan; tanpa aksen KDE dipakai oranye
  bawaan. Override untuk pengujian: `RADIAL_DECK_FORCE_ACCENT`.
- Filter roda folder memakai ranking bersama `search::match_rank`: label
  diawali query, lalu label mengandung, lalu path mengandung.
- Golden-image test untuk ketiga render roda (Convert/Tools/Actions) di
  `scripts/ui-smoke.sh` (referensi di `scripts/golden`, regen via
  `RADIAL_DECK_GOLDEN_REGEN=1`).
- `docs/manual.md`: referensi lengkap satu halaman untuk pengguna dan agen AI.

### Changed

- Kompresi ZIP pindah ke backend zlib (`deflate-zlib`); zopfli yang membuat
  penulisan ZIP ~10x lebih lambat tidak lagi ada di dependency graph.
- Resolusi biner eksternal di-cache per proses (`find_tool`); Settings →
  Recheck menyegarkannya. Roda Tools dan konversi tidak memindai `PATH`
  berulang-ulang lagi.
- Loop pekerjaan konversi/tool menerima `WorkerEvent` dari satu channel yang
  di-`await` (tanpa polling 60 ms); thread yang panic kini terdeteksi.
- Nama biner eksternal dikonsolidasi ke `deck-core::binaries` satu sumber.
- Notifikasi desktop dilayani satu thread worker; Float shelf dihancurkan saat
  ditutup; scan dependensi hanya saat Settings dibuka atau Recheck.
- Hover roda memaafkan klik ±6 px di tepi luar cincin.
- Label roda memakai satu `PangoLayout` yang dipakai ulang (render
  terverifikasi identik piksel oleh golden test).

### Fixed

- Stderr child process kini benar-benar sampai ke panel progres (thread
  pembaca sebelumnya berada di luar job scope sehingga dibuang diam-diam).
- Penghitung progres batch kini bertambah (sebelumnya selalu `done: 1`).
- Ekstensi `.tif/.ppm/.pgm/.pbm` kini benar-benar bisa dikonversi (daftar UI
  dan engine diturunkan dari satu sumber).
- Output ditulis lewat `create_new` sehingga operasi serentak tidak mungkin
  saling menimpa; `history.json` ditulis atomik dengan mode 0600.
- `capture()` (ffprobe/identify) bisa dibatalkan dan tidak lagi memblokir
  tanpa batas.
- Probe KDE Connect tidak lagi berjalan di thread utama saat mode Actions
  dibuka.
- Panic kini tercatat di log harian (sebelumnya crash diam-diam membunuh tray
  dan dua hotkey).
- Kontras WCAG: border fokus terlihat di kedua tema, teks mode aktif dan label
  kunci ≥4.5:1, muted wedge gelap ≥4.5:1.
- `convert --list | head` dan `--help | head` tidak lagi panic karena SIGPIPE.

## [0.1.0] - 2026-10-28

Rilis Linux perdana: roda konversi tiga mode (Convert/Tools/Actions), 26 file
tool dengan form parameter, roda folder proyek multi-project, dua hotkey
GlobalShortcuts portal, layer-shell Wayland, Float shelf, tray + Undo, CLI
batch `convert`, config schema 5, installer user-local, PKGBUILD.

[0.2.0]: https://github.com/ReyyIchiro/radialdeck/releases/tag/v0.2.0
[0.1.0]: https://github.com/ReyyIchiro/radialdeck/releases/tag/v0.1.0
