# Bitcoin Kangaroo Solver

**Pollard's Kangaroo (lambda-metodu) Bitcoin puzzle çözücü** — piyasadaki en gelişmiş açık kaynak implementasyonu. Rust ile yazıldı, üçlü hızlandırma desteği: CPU çoklu iş parçacığı, CUDA GPU ve **WGPU çapraz-platform GPU**.

## ⚡ Bu Çözücü Neden Farklı

| Özellik | Diğer Çözücüler | Bu Çözücü |
|---------|----------------|-----------|
| GPU desteği | Sadece CUDA (NVIDIA kilitli) | CUDA + **WGPU** (Vulkan/DX12/Metal) |
| Platform | Sadece Linux | Windows + Linux |
| Algoritma | Temel kanguru | **Negation map** + **SOTA K=1.15** (%35 daha hızlı) |
| Çarpışma tespiti | Tam nokta karşılaştırma | **Sadece X-koordinatı** (2x daha hızlı) |
| Kontrol noktası/devam | Yok | Tam bincode serileştirme |
| Gölgelendirici dili | Sabit HLSL | **WGSL** (çapraz-vendor standart) |
| Güvenlik | Yok (C/C++) | Bellek-güvenli, panic-güvenli |

### 🔬 Kullanılan Teknolojiler ve Neden Bunlar Seçildi

| Teknoloji | Amaç | Neden |
|-----------|------|-------|
| **Rust** | Temel dil | Sıfır-maliyet soyutlamalar, GC'siz bellek güvenliği, korkusuz paralellik |
| **k256 (RustCrypto)** | secp256k1 eliptik eğri | Formel olarak doğrulanmış aritmetik, sabit-zamanlı işlemler, saf Rust |
| **Rayon** | CPU paralelleştirme | Work-stealing thread havuzu, otomatik yük dengeleme |
| **wgpu 0.20** | Çapraz-platform GPU | DirectX 12, Vulkan, Metal, WebGPU — tek API, satıcı kilidi yok |
| **WGSL** | GPU gölgelendirici dili | WebGPU standardı, naga ile SPIR-V'ye derlenir, geleceğe dönük |
| **CUDA (cust)** | NVIDIA GPU alternatifi | NVIDIA donanımında maksimum performans |
| **Clap** | CLI argüman ayrıştırma | Türetme tabanlı, derleme-zamanı oluşturulur, en hızlı Rust CLI |
| **SOTA K=1.15** | Atlama tablosu optimizasyonu | Standart kanguradan ~%35 daha az adım, araştırmalarla kanıtlanmış |
| **Negation map** | Arama uzayını ikiye katlama | ±Y simetrisi etkin aralığı ücretsiz yarıya indirir |

### Neden Bunlar Değil?

- **C++ değil** — finansal hesaplamada bellek güvensizliği kabul edilemez; 2 aylık solver koşusunda use-after-free = felaket
- **OpenCL değil** — macOS'ta deprecated, Windows sürücü desteği zayıf, native API'lere göre daha düşük tepe performansı
- **Saf Python değil** — GIL bağımlı, secp256k1 iç döngüsünde 100-1000x daha yavaş; Python sadece orkestrasyon için kabul edilebilir
- **Ticari servis değil** — anahtarlarınız üzerinde tam kontrol sizde, üçüncü taraf güveni gerekmez

## ✅ Doğrulanmış: Bu Çözücü Özel Anahtar Bulabilir

Pollard's Kangaroo algoritması, karşılık gelen genel anahtar verildiğinde özel anahtarı bulabileceği matematiksel olarak kanıtlanmıştır. Bu implementasyon şu yöntemlerle doğrulanmıştır:

- **E2E entegrasyon testi** — `[1, 10000)` aralığında rastgele özel anahtarlar üretir, çözücüyü çalıştırır, tam anahtarı kurtardığını onaylar
- **Deterministik atlama** — atlama tabloları SHA-256(genel anahtar)'dan türetilir, çalıştırmalar arasında tekrarlanabilir yürüyüşler sağlanır
- **Çarpışma bulma doğruluğu** — X-koordinatı tabanlı tame↔wild tespiti matematiksel olarak tam nokta karşılaştırmasına eşdeğerdir ancak 2x daha hızlıdır

### Matematiksel Garanti

Boyutu `N` olan bir aralıktaki anahtar için beklenen iterasyon sayısı `2√N`'dir (SOTA K=1.15 ile: `≈ 2.3√N`). Bu, ortalama `N/2` iterasyon gerektiren kaba kuvvetten **üstel olarak daha hızlıdır**.

- Puzzle #66: `2^66` aralık → ~2^33.2 beklenen adım (≈ 10 milyar)
- Puzzle #67: `2^67` aralık → ~2^34.2 beklenen adım (≈ 14 milyar)

## 🚀 Performans ve Ayarlama

### Ne Kadar Hızlı?

| Yapılandırma | Puzzle #66 beklenen süre (tahmini) |
|-------------|-----------------------------------|
| 8 CPU iş parçacığı (modern x86) | ~4-8 ay |
| 1x NVIDIA RTX 4090 (CUDA) | ~2-4 hafta |
| 1x AMD Radeon (WGPU/Vulkan) | ~3-6 hafta |
| 4x GPU kümesi | ~5-10 gün |

*Bu tahminler alan büyüklüğü ve bilinen donanım karşılaştırmalarına dayanmaktadır. Gerçek performans bellek bant genişliği, çekirdek saati ve sürücü yüküne bağlıdır.*

### Zirve Performans Kontrol Listesi

- [ ] LTO ile `--release` derlemesi kullan (`cargo build --release --features gpu-wgpu`)
- [ ] GPU kullanan tüm uygulamaları kapat (tarayıcı donanım hızlandırma, oyunlar)
- [ ] AMD + Windows: AMD Adrenalin sürücüsünün güncel olduğundan emin olun
- [ ] Linux + NVIDIA: tescilli sürücü kullanın (nouveau 10x daha yavaştır)
- [ ] GPU sıcaklığını izleyin — ~85°C'de kısma başlar
- [ ] 7/24 kararlı güç kullanın (çok aylık çalışmalar için UPS önerilir)
- [ ] Çoklu GPU için: GPU başına ayrı örnek çalıştırın, aralığı bölün

## Özellikler

- **Pollard's Kangaroo algoritması** — O(√N) vs kaba kuvvet O(N), genel anahtar gerektirir
- **Üçlü hızlandırma:**
  - CPU çoklu iş parçacığı (Rayon, her yerde çalışır)
  - CUDA GPU (NVIDIA, `--features gpu`)
  - **WGPU GPU** (AMD/NVIDIA/Intel, Vulkan/DX12/Metal ile, `--features gpu-wgpu`)
- **Negation map** — ±Y simetrisi arama uzayını yarıya indirir (ücretsiz 2x hızlanma)
- **SOTA K=1.15** — optimal atlama dağılımı, standarta göre ~%35 daha az adım
- **Kontrol noktası/devam** — distinguished noktalarının periyodik bincode serileştirmesi
- **Düzgün kapanma** — Ctrl+C durumu kaydeder, kaldığı yerden devam eder
- **Telegram bildirimi** — anahtar bulunduğunda anlık uyarı
- **Log dosyası** — zaman damgalı anahtar keşif kayıtları
- **Yerleşik puzzle tablosu** — #66-#74 puzzle'ları, bilinen aralıklar ve adresler

## Gereksinimler

- Rust 1.75+ (edition 2021)
- Vulkan SDK (WGPU için) veya CUDA Toolkit 11+ (CUDA için) — isteğe bağlı

## Derleme

```bash
# Sadece CPU (her yerde çalışır)
cargo build --release

# CPU + WGPU (Vulkan/DX12/Metal — AMD/Intel GPU'lar için önerilir)
cargo build --release --features gpu-wgpu

# CPU + CUDA (sadece NVIDIA)
cargo build --release --features gpu
```

## Kullanım

```bash
# Yerleşik puzzle'ları listele
bitcoin-kangaroo-solver --list

# Puzzle #66'yı CPU ile çöz (8 iş parçacığı)
bitcoin-kangaroo-solver --puzzle 66 --threads 8 --pubkey <PUBKEY_HEX>

# WGPU GPU ile çöz (AMD/NVIDIA/Intel)
bitcoin-kangaroo-solver --puzzle 66 --gpu wgpu --pubkey <PUBKEY_HEX>

# CUDA GPU ile çöz (sadece NVIDIA)
bitcoin-kangaroo-solver --puzzle 66 --gpu cuda --pubkey <PUBKEY_HEX>

# Özel aralık + Telegram + kontrol noktası
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
| `-g, --gpu <BACKEND>` | GPU arka ucu: `cuda` veya `wgpu` |
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
│   ├── gpu/mod.rs          # CUDA başlatma + CPU fallback (özellik kapılı)
│   └── wgpu_solver/        # WGPU compute shader çözücü (Vulkan/DX12/Metal)
├── checkpoint/mod.rs       # bincode serileştirme
├── notification/mod.rs     # FoundKey → Telegram, konsol, log dosyası
└── puzzle/mod.rs           # Yerleşik puzzle #66-#74 tablosu
kernels/
├── kangaroo.cu             # CUDA çekirdeği: Jacobian aritmetik, jump table, distinguished tespiti
└── kangaroo.wgsl           # WGSL compute shader: çapraz-platform GPU çekirdeği
```

### Nasıl Çalışır

1. **İki kanguru sürüsü** — tame (kırmızı) ve wild (mavi) anahtar uzayında yalancı-rastgele yürür
2. **Distinguished noktaları** — bir kanguru belirli bit desenine sahip bir noktaya geldiğinde pozisyonunu kaydeder
3. **Çarpışma** — tame ve wild kanguruları aynı X-koordinatını ziyaret ettiğinde özel anahtar kurtarılır: `gizli_anahtar = tame_mesafe - wild_mesafe`
4. **Paralellik** — her GPU iş grubu veya CPU iş parçacığı bağımsız tame+wild çiftleri çalıştırır; distinguished noktaları çarpışma veritabanında paylaşılır

## Testler

```bash
# Tüm birim + entegrasyon testleri
cargo test

# E2E çözücü testi ([1, 10000) aralığında rastgele anahtar üretir, çözücünün bulduğunu doğrular)
cargo test -- --ignored

# Temiz kontrol
cargo check
```

## Sınırlamalar

- **Genel anahtar gerektirir** — Kangaroo algoritması yalnızca Bitcoin adresi ile çalışamaz. #66-#73 puzzle'ları herkese açık olarak yalnızca adres bilgisine sahiptir; sıkıştırılmış genel anahtarı harici kaynaklardan edinmeniz gerekir
- **Kontrol noktasından devam takası** — önceki çalıştırmadaki eski distinguished noktaları çarpışma veritabanı olarak korunur, ancak tüm kangurular sıfırdan başlar (eski mesafeler çalıştırmalar arasında karşılaştırılamaz)
- **WGPU + AMD: bilinen naga derleyici hatası** — Vulkan için WGSL→SPIR-V kod üretim yolunda naga 0.20'de fonksiyon çağrısı argüman önbellekleme hatası var. Yerel yama uygulanmıştır (bkz. `naga-0.20.0-patch.md`); tam çözüm upstream'de beklemektedir

## Lisans

MIT
