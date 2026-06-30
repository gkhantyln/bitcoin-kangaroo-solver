# Bitcoin Kangaroo Solver

Pollard's Kangaroo (lambda-metodu) tabanlı Bitcoin puzzle çözücü — Rust ile yazıldı, CPU çoklu iş parçacığı ve isteğe bağlı CUDA GPU hızlandırması.

## Özellikler

- **Pollard's Kangaroo algoritması** — O(√N) vs kaba kuvvet O(N), genel anahtar gerektirir
- **CPU çoklu iş parçacığı** — Rayon ile thread başına tame+wild kanguru çiftleri
- **CUDA GPU çözücü** — Jacobian aritmetik çekirdeği, `--features gpu` ile etkinleştirilir
- **Kontrol noktası/devam** — Distinguished noktalarının periyodik bincode serileştirmesi
- **Düzgün kapanma** — SIGINT (Ctrl+C) kontrol noktasını kaydeder ve temiz çıkış yapar
- **Telegram bildirimi** — Bloklayan reqwest ile bulunan anahtar ilgili sohbete gönderilir
- **Log dosyası** — Bulunan anahtarları zaman damgasıyla dosyaya ekler
- **Yerleşik puzzle tablosu** — #66-#74 arası puzzle'lar, bilinen aralıklar ve adresler

## Gereksinimler

- Rust 1.75+ (edition 2021)
- CUDA toolkit 11+ (sadece GPU derlemesi için)

## Derleme

```bash
# Sadece CPU (her yerde çalışır)
cargo build --release

# CPU + GPU (CUDA toolkit gerektirir)
cargo build --release --features gpu
```

Çıktı: `target/release/bitcoin-kangaroo-solver.exe`

## Kullanım

```bash
# Yerleşik puzzle'ları listele
bitcoin-kangaroo-solver --list

# Puzzle #66'yı 8 CPU iş parçacığı ile çöz
bitcoin-kangaroo-solver --puzzle 66 --threads 8 --pubkey <Sıkıştırılmış_Genel_Anahtar_Hex>

# Özel aralık + Telegram bildirimi + kontrol noktası
bitcoin-kangaroo-solver ^
  --start-range 0000000000000000000000000000000000000000000000000000000000000001 ^
  --end-range   0000000000000000000000000000000000000000000000000000000001000000 ^
  --pubkey 02<64_Hex_Karakter> ^
  --address 1PVoXoTNaGWtnFfGAhf1RMycFUssCPnCGE ^
  --checkpoint puzzle.cp ^
  --telegram-bot-token <BOT_TOKEN> ^
  --telegram-chat-id <CHAT_ID>

# Kontrol noktasından devam et
bitcoin-kangaroo-solver --puzzle 66 --checkpoint puzzle.cp --pubkey <HEX>

# GPU çözücü (CUDA gerektirir)
bitcoin-kangaroo-solver --puzzle 66 --gpu --pubkey <HEX>
```

### Seçenekler

| Bayrak | Açıklama |
|--------|----------|
| `--puzzle <N>` | Yerleşik puzzle 66-74 (aralık + adres belirler) |
| `--start-range <HEX>` | Özel başlangıç aralığı (32 bayt hex) |
| `--end-range <HEX>` | Özel bitiş aralığı (32 bayt hex) |
| `--address <ADDR>` | Hedef Bitcoin adresi (sadece görüntüleme) |
| `--pubkey <HEX>` | Hedef sıkıştırılmış genel anahtar (66 hex, 02/03 öneki) — **ZORUNLU** |
| `-t, --threads <N>` | CPU iş parçacığı sayısı (varsayılan: çekirdeklerin yarısı) |
| `-g, --gpu` | CUDA GPU çözücüyü etkinleştir |
| `-c, --checkpoint <YOL>` | Devam için kontrol noktası dosyası |
| `--checkpoint-interval <N>` | Kontrol noktası kaydetme aralığı (saniye, varsayılan: 300) |
| `--distinguished-bits <N>` | Distinguished noktası bit sayısı (varsayılan: 20) |
| `--telegram-bot-token <T>` | Telegram bot token'ı |
| `--telegram-chat-id <ID>` | Telegram sohbet ID'si |
| `--log <YOL>` | Bulunan anahtarlar için log dosyası |
| `-l, --list` | Yerleşik puzzle'ları listele |

## Mimari

```
src/
├── main.rs                 # CLI: clap argümanları → config → çözücü yönlendirme
├── lib.rs                  # Modül dışa aktarımları, INTERRUPTED bayrağı
├── kangaroo/
│   ├── point.rs            # secp256k1: Scalar, ProjectivePoint, jump table, adres türetme
│   ├── walk.rs             # KangarooWalk: step(), is_distinguished(), affine X önbelleği
│   ├── collision.rs        # CollisionFinder: X-koordinatı bazlı tame↔wild tespiti
│   ├── params.rs           # KangarooParams: tüm çözücü konfigürasyonu
│   └── distinguished.rs    # SHA256 önde giden sıfır bitleri ile distinguished tespiti
├── solver/
│   ├── mod.rs              # Solver trait'i
│   ├── cpu/mod.rs          # Rayon paralel çözücü, paylaşılan çarpışma veritabanı
│   └── gpu/mod.rs          # CUDA başlatma + CPU fallback (özellik kapılı)
├── checkpoint/mod.rs       # bincode serileştirme
├── notification/mod.rs     # FoundKey → Telegram, konsol, log dosyası
└── puzzle/mod.rs           # Yerleşik puzzle #66-#74 tablosu
kernels/
└── kangaroo.cu             # CUDA çekirdeği: Jacobian aritmetik, jump table, distinguished tespiti
```

### Nasıl Çalışır

1. **İki kanguru sürüsü** — tame (kırmızı) ve wild (mavi) anahtar uzayında yalancı-rastgele yürür
2. **Distinguished noktaları** — bir kanguru belirli bit desenine sahip bir noktaya geldiğinde pozisyonunu kaydeder
3. **Çarpışma** — tame ve wild kanguruları aynı X-koordinatını ziyaret ettiğinde özel anahtar kurtarılır: `gizli_anahtar = tame_mesafe - wild_mesafe`
4. **Paralellik** — her thread bağımsız tame+wild çiftleri çalıştırır; distinguished noktaları `Arc<Mutex<Vec<...>>>` ile paylaşılır

## Testler

```bash
# Tüm birim + entegrasyon testleri (~40sn)
cargo test

# E2E çözücü testi ([1, 10000) aralığında rastgele anahtar üretir, çözücünün bulduğunu doğrular)
cargo test -- --ignored

# Temiz kontrol
cargo check
```

## Sınırlamalar

- **Genel anahtar gerektirir** — Kangaroo algoritması yalnızca Bitcoin adresi ile çalışamaz. #66-#73 puzzle'ları herkese açık olarak yalnızca adres bilgisine sahiptir; sıkıştırılmış genel anahtarı harici kaynaklardan edinmeniz gerekir
- **Kontrol noktasından devam takası** — önceki çalıştırmadaki eski distinguished noktaları çarpışma veritabanı olarak korunur, ancak tüm kangurular sıfırdan başlar (eski mesafeler çalıştırmalar arasında karşılaştırılamaz)
- **GPU test edilmedi** — CUDA yolu yazıldı ve derleniyor ancak donanımda doğrulanmayı bekliyor

## Lisans

MIT
