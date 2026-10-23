# Status Proyek

Dokumen hidup. Perbarui bagian **Snapshot**, **Verifikasi**, dan **Antrean** setiap kali
perilaku berubah. Riwayat detail tetap di git; dokumen ini menjawab: apa yang benar-benar
sudah terbukti, apa yang belum, dan apa berikutnya.

## Snapshot

- Tanggal: 2026-10-23
- Versi: 0.1.0 (schema config 2)
- Platform terverifikasi: CachyOS, KDE Plasma 6, Wayland, GTK 4.20 / layer-shell
- Fokus: Linux tahap 1 selesai; Windows/macOS belum dimulai

## Ringkasan

Tahap Linux sudah mencakup dua alur utama:

1. **Quick actions** — roda konversi file/arsip, drag & drop, multi-kategori, konversi
   lokal, service menu Dolphin.
2. **Project folders** — roda folder per project, filter, keyboard, terminal, clipboard,
   multi-project, kelola folder (rename, warna, urutan, buka/salin), dan hotkey terpisah.

Keduanya memakai overlay layer-shell dengan panel yang mengikuti tema sistem. Dua shortcut
global didaftarkan melalui XDG GlobalShortcuts portal.

## Verifikasi terakhir

| Pemeriksaan | Hasil |
|---|---|
| `cargo fmt --all` | bersih |
| `cargo clippy --workspace --all-targets -- -D warnings` | bersih |
| `cargo test --workspace` | 45 lulus (40 core + 5 app) |
| `cargo build --release --locked` | lulus |
| Dua shortcut portal | `Ctrl+Alt+Space` → quick actions, `Ctrl+Alt+P` → project folders |
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
| Binary release | 6.5 MiB, stripped |

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
- Settings: dua shortcut, pintasan ke Manage, path log + tombol buka log.
- Log harian di `$XDG_STATE_HOME/radial-deck/logs`, retensi 7 file.
- Config schema 2 + migrasi aditif (`projects_hotkey`); duplikat shortcut diperbaiki.

## Known issues / batasan

- Wayland tidak memberi posisi kursor global; panel selalu di tengah output aktif.
- X11 fallback belum memposisikan overlay di kursor (jendela biasa); posisi overlay
  bergantung pada window manager.
- GTK masih mencatat warning `slider min width -2` saat window Manage dibuka; berasal dari
  tema sistem, bukan CSS aplikasi.
- Flatpak masih WIP; konverter host belum diakses dari sandbox.
- File tools Tangerine (crop/trim/split/merge/compress dengan preview) belum ada.
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
| P1 | File tools inti (compress/crop/trim) | preview + eksekusi lokal + tes |
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
