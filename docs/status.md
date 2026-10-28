# Status Proyek

Dokumen hidup. Perbarui bagian **Snapshot**, **Verifikasi**, dan **Antrean** setiap kali
perilaku berubah. Riwayat detail tetap di git; dokumen ini menjawab: apa yang benar-benar
sudah terbukti, apa yang belum, dan apa berikutnya.

## Snapshot

- Tanggal: 2026-10-28
- Versi: 0.1.0 (schema config 5)
- Platform terverifikasi: CachyOS, KDE Plasma 6, Wayland, GTK 4.20 / layer-shell
- Fokus: Linux tahap 1 selesai; Windows/macOS belum dimulai

## Ringkasan

Tahap Linux sudah mencakup dua alur utama:

1. **Quick actions** — satu roda dengan tiga mode:
   - *Convert*: konversi file/arsip, drag & drop, multi-kategori, service menu Dolphin.
   - *Tools*: 26 file tool lokal dengan form parameter (compress, crop, edit, annotate,
     frame, redact, PDF, video, audio, arsip).
   - *Actions*: Copy, Move, Copy Path, Terminal, Send (KDE Connect), Float.
2. **Project folders** — roda folder per project, filter, keyboard, terminal, clipboard,
   multi-project, kelola folder (rename, warna, urutan, buka/salin), dan hotkey terpisah.

Ditambah **Float shelf**: jendela mengambang berisi file, tiap baris drag source nyata.

Keduanya memakai overlay layer-shell dengan panel yang mengikuti tema sistem. Dua shortcut
global didaftarkan melalui XDG GlobalShortcuts portal.

## Verifikasi terakhir

| Pemeriksaan | Hasil |
|---|---|
| `cargo fmt --all` | bersih |
| `cargo clippy --workspace --all-targets -- -D warnings` | bersih |
| `cargo test --workspace` | 85 lulus (69 core + 16 app) |
| `cargo build --release --locked` | lulus |
| Dua shortcut portal | `Ctrl+Alt+Space` → quick actions, `Ctrl+Alt+Q` → project folders |
| Klik luar lalu satu kali hotkey | membuka overlay kembali (regresi fixed) |
| `Esc`, klik luar, hotkey toggle | state controller dan window konsisten |
| Roda folder satu item | target cincin penuh, label tidak terpotong |
| Rail project | 5 tab + overflow `+3`; tab aktif dan fokus keyboard terlihat |
| Tab dari roda | fokus pindah ke tab pertama, border fokus putih tampil |
| Escape saat query aktif | query dibersihkan; Escape berikutnya menutup overlay |
| Salin path | `Shift+Enter` menyalin `/tmp` ke clipboard |
| Manage 8 project / 4 folder | rail, warna, rename, tambah folder, urutan, badge missing |
| Rename project | dialog, Enter, config tersimpan, status “Project renamed” |
| Project color | popover 12 warna, pilihan tersimpan ke config |
| Add folder | chooser native, path tersimpan, status “Folder added” |
| Roda konversi PNG → JPG | terverifikasi pada rilis sebelumnya |
| Konversi WAV → MP3 | terverifikasi pada rilis sebelumnya |
| Arsip ZIP/TAR/GZIP + extract | terverifikasi pada rilis sebelumnya |
| Idle RAM (release, hidden, Cairo) | 53 MB RSS saat start |
| RAM setelah window pertama | 67–87 MB RSS |
| Window settings ditutup & dibuka ulang | tidak crash; referensi dibersihkan |
| 26 file tool via `examples/tool_smoke` | 24 sukses; 2 penolakan input benar (`extract_archive` pada `.md`) |
| `join_videos` dua klip | `joined.mp4` setelah path concat dibuat absolut |
| Form Compress (kualitas 60) dari UI | `photo-compressed.png` 96×64 tertulis |
| Live-modifier switching (Shift/Super/Ctrl) | mode berganti instan, sumber file ter-seed dipertahankan; dapat dimatikan di Settings |
| Progres + Cancel | stderr FFmpeg di-stream ke panel; cancel menghentikan child dan menghapus output parsial |
| Bug: panic setelah tool sukses | `record_history` meminjam riwayat dua kali; diperbaiki, smoke test kini menjaga liveness setelah run |
| Lokasi output | `alongside` / `subfolder` / `folder` diuji lewat unit test dan konversi nyata |
| Urutan tool per kategori | unit test untuk urutan, filter kategori, dan daftar kosong |
| Riwayat + Undo | rekaman tersimpan; Undo menghapus output dan menyegarkan tray |
| CLI `convert` | exit 0/2, `--into`, nama bentrok `-1`, `--help`; tidak menginisialisasi GTK, jalan tanpa display |
| Mode Actions: Copy Path | clipboard berisi path absolut |
| `scripts/ui-smoke.sh` (KWin virtual, layar nyata tidak tersentuh) | 40/40: overlay Wayland ter-map & tergambar, form Compress menulis file, Float shelf 340×420, `Esc` menutup, live-modifier, navigasi 6 halaman Settings (termasuk daftar folder proyek dan tema gelap), CLI batch; artefak per-lane `app-N.log` |
| `ui-smoke.sh` isolasi bus | bus D-Bus privat; 0 proses tersisa, keluar 25 detik tanpa menggantung pipe |
| Idle CPU (release, hidden) | 0,5% setelah 20 detik |
| Mode Actions: Float | shelf 340×420, baris `photo.png`, tombol tutup per baris |
| Roda Tools di Wayland | screenshot: 8 tool gambar untuk kategori Images |
| `Ctrl+A` di form | memilih teks; mode tidak berpindah ke Actions |
| Overlay X11 | fullscreen pada monitor pointer (1920×1080 eDP-1) |
| Layer-shell dua monitor | roda muncul di monitor tempat pointer berada |
| Binary release | 6.9 MiB, stripped |

Catatan: konversi tidak berubah pada iterasi ini; hasil sebelumnya tetap berlaku selama
tidak ada perubahan pada `deck-core::convert`.

## Perubahan penting iterasi ini

- Memisahkan shortcut global: `toggle` dan `projects`; settings/tray menampilkan keduanya.
- Memperbaiki bug overlay “tidak muncul”: handler klik luar/Escape tidak lagi menyembunyikan
  root/window secara langsung tanpa memperbarui state controller.
- Overlay kini punya flag visibilitas eksplisit, recreation bila window destroyed, verifikasi
  map + retry, dan mutual exclusion antar-overlay.
- Roda folder memakai panel `.rd-panel` + palet roda konversi; single-folder menjadi satu
  target cincin penuh (bukan kartu), dengan alasan folder hilang di tengah.
- Rail project baru: maksimal lima tab, project aktif selalu terlihat, sisanya di menu `+N`;
  tab aktif memakai border oranye, fokus keyboard memakai border putih.
- `Tab` dari roda memindahkan fokus ke tab pertama; `Enter` memilih project. Escape saat
  query aktif membersihkan query lebih dulu.
- Label wedge ditempatkan di tengah ring dengan font samping lebih kecil; tidak lagi
  menabrak pusat atau tepi luar.
- Klik tengah wedge menyalin path; pusat membuka Manage (atau folder saat hanya satu).
- Renderer Cairo dipaksa sebelum GTK init; RSS window pertama turun dari ~115 MB ke ~68 MB
  tanpa ketergantungan GPU. `GSK_RENDERER` tetap bisa di-override.
- Window manage/settings dihancurkan saat ditutup agar widget tree dilepas; overlay roda
  disembunyikan dan dipakai ulang.
- Manage: rename folder, warna folder/project, buka/terminal/salin path, konfirmasi hapus,
  empty state, reorder; delete project dinonaktifkan saat hanya satu project.
- Settings: sidebar 6 halaman (General, Shortcuts, Projects, Tool wheel, Output, Advanced),
  daftar folder per proyek, dua shortcut, pintasan ke Manage, path log + tombol buka log.
- Log harian di `$XDG_STATE_HOME/radial-deck/logs`, retensi 7 file.
- Config schema 3: migrasi aditif `projects_hotkey`, default project pindah ke
  `CTRL+ALT+Q`, lalu duplikat shortcut diperbaiki.
- Garis diagonal pada roda aksi diperbaiki: tiap label memakai path Cairo baru sehingga
  path teks tidak tersambung ke wedge berikutnya.

## Known issues / batasan

- Wayland tidak memberi posisi kursor global; panel berada di tengah monitor yang berisi
  pointer (`set_monitor`), bukan di titik kursor.
- X11 fallback mengisi seluruh monitor pointer (bukan jendela kecil di kursor); posisi
  absolut tidak tersedia di GTK4 lintas backend.
- GTK masih mencatat warning `slider min width -2` saat window Manage dibuka; berasal dari
  tema sistem, bukan CSS aplikasi.
- Flatpak masih WIP; konverter host belum diakses dari sandbox.
- File tool berjalan sinkron di thread kerja; belum ada preview sebelum eksekusi dan
  belum ada progres per persen untuk file besar.
- Drag keluar dari baris Float bergantung pada file manager tujuan yang menerima
  `text/uri-list`.
- Windows dan macOS belum diimplementasikan; lihat `docs/roadmap.md`.
- Portal KDE meminta konfirmasi satu kali untuk shortcut baru (`projects`).
- Jika compositor tidak mendukung layer-shell, aplikasi memakai jendela biasa; fokus
  exclusive tidak dijamin.

## Antrean berikutnya (prioritas)

| Prioritas | Item | Kriteria selesai |
|---|---|---|
| P0 | Uji settings ganti shortcut lewat portal | binding baru aktif dan tersimpan |
| P1 | Uji tray + autostart setelah perubahan label shortcut | menu/tooltip benar, login autostart tetap toggle |
| P1 | Audit CSS switch GTK (`slider min width`) | tidak ada warning GTK saat settings dibuka |
| P1 | Preview sebelum eksekusi tool | form menampilkan hasil sementara sebelum menulis |
| P1 | Progres per file untuk tool panjang | status per file, bisa dibatalkan |
| P1 | Windows shell (Tauri v2) memakai `deck-core` | overlay, dua hotkey, tray, launch folder, installer |
| P2 | macOS shell | `LSUIElement`, panel non-activating, dua hotkey, tray, notarization |
| P2 | Flatpak | konverter tersedia atau `flatpak-spawn --host` dengan izin jelas |
| P3 | Tema/aksen KDE (`kdeglobals`) untuk panel | warna accent mengikuti sistem, fallback aman |
| P3 | Accessibility audit | fokus keyboard, label screen reader, kontras WCAG AA |

## Cara memperbarui

1. Jalankan perintah verifikasi di `AGENTS.md`.
2. Catat hasil nyata, bukan asumsi; tandai item yang belum diuji.
3. Perbarui tanggal dan versi.
4. Tambahkan known issue baru; hapus yang sudah tidak berlaku.
5. Jangan menghapus riwayat keputusan di `docs/architecture.md` tanpa alasan.
