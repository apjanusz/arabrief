# AraBrief

Aplikacja desktopowa Windows: Tauri 2, React 19, TypeScript 6, Vite 8 i Tailwind CSS 4.
Docelowo okresowo ścisza muzykę innej aplikacji, odtwarza krótką audycję AI i przywraca
wcześniejszą głośność. Muzyka nigdy nie jest pauzowana.

## Obecny zakres

Minimalny polski ekran ze statusem oraz interfejsy modułów. Nie ma jeszcze integracji
Windows Audio, RSS, API, syntezy mowy, zapisu ustawień ani działającego harmonogramu.
Status ekranu jest statyczny. Komendy IPC są propozycją kontraktu, nie działającym API.

## Uruchomienie na Windows

Wymagania: Node.js 22.12+ (lub obsługiwana nowsza wersja LTS), npm, Rust stable
z targetem MSVC, Visual Studio Build Tools z C++ i Windows SDK oraz WebView2 Runtime.
Narzędzia muszą być dostępne w PATH.

```powershell
npm ci
npm run tauri dev
```

Sam interfejs w przeglądarce: `npm run dev`. Kompilacja frontendu: `npm run build`.
Pełna kompilacja i instalator: `npm run tauri build`.
Kontrole Rust: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml` oraz
`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
Repozytorium nie zawiera jeszcze test runnera ani lintera JS; build sprawdza TypeScript.

Tailwind jest podłączony przez `@tailwindcss/vite` i `@import "tailwindcss"` w App.css,
zgodnie z https://tailwindcss.com/docs/installation/using-vite. Nie wymaga PostCSS ani
osobnego pliku konfiguracji Tailwind.

## Struktura i odpowiedzialności

| Lokalizacja | Odpowiedzialność |
| --- | --- |
| `src/App.tsx`, `src/App.css` | Minimalny ekran i style Tailwind |
| `src/domain/models.ts` | Dane wiadomości, scenariusza, audio, ustawień i stanu |
| `src/modules/audio/AudioController.ts` | Port do natywnego Windows Audio: sesje, ściszenie, odtwarzanie i przywrócenie |
| `src/modules/news/NewsProvider.ts` | Pobieranie i normalizacja wiadomości |
| `src/modules/content/ContentGenerator.ts` | Generowanie scenariusza z zachowaniem źródeł |
| `src/modules/speech/SpeechProvider.ts` | Synteza i zwalnianie lokalnego zasobu mowy |
| `src/modules/scheduler/BroadcastScheduler.ts` | Interwały, orkiestracja, anulowanie, stan; jedna audycja naraz |
| `src/modules/settings/SettingsRepository.ts` | Walidacja, wersjonowanie i zapis ustawień bez sekretów |
| `src/platform/tauri/contracts.ts` | Typy przyszłego mostu IPC, niezależne od UI |
| `src-tauri/src/lib.rs` | Punkt kompozycji przyszłych adapterów natywnych |

Zależności: UI → scheduler → interfejsy pozostałych modułów. Konkretne adaptery będą
wstrzykiwane w punkcie kompozycji; domena nie importuje React ani Tauri. Bez backendu
serwerowego. Ustawienia przechowują preferencje, nigdy klucze API.

Docelowy przebieg: pobranie wiadomości → scenariusz → synteza → ściszenie wybranych
sesji → odtworzenie → przywrócenie głośności → zwolnienie zasobu. Przygotowanie treści
odbywa się przed ściszeniem. Scheduler musi przywracać audio w `finally`, także po
anulowaniu lub błędzie, i nie dopuszczać nakładania audycji. `stop()` czeka na sprzątanie.

## Kontrakt natywny (plan)

Nazwy komend i typy danych są w `src/platform/tauri/contracts.ts`. Nie są obecnie
zarejestrowane w Rust i nie wolno ich wywoływać przed implementacją adaptera.

| Komenda | Argumenty invoke | Wynik |
| --- | --- | --- |
| `audio_list_sessions` | brak | lista sesji (bez sesji AraBrief) |
| `audio_duck` | `{ request: { sessionIds, volume } }` | `{ restorationToken }` |
| `audio_restore` | `{ request: { restorationToken } }` | null |
| `audio_play` | `{ request: { resourceId } }` | null po zakończeniu odtwarzania |
| `audio_stop` | brak | null po zatrzymaniu głosu |

Rust ma używać serde camelCase dla DTO i `Result<T, NativeError>`; błędy odrzucają
Promise z `{ code, message }`. Kontrakty TS nie walidują danych w czasie wykonania.
Natywna strona musi walidować sesje, skończoną głośność w [0,1] i identyfikatory zasobów.
Token przywrócenia jest nieprzezroczysty: snapshot pozostaje w Rust. Ściszenie nigdy nie
zwiększa głośności; operacja częściowo nieudana odtwarza snapshot przed zwróceniem błędu.
Przywracanie jest idempotentne. Zakończona sesja nie może blokować przywracania pozostałych.
Rust musi również zapewnić odzyskiwanie głośności przy zamknięciu/awarii warstwy UI.
`audio_stop` kończy aktywne `audio_play`; scheduler traktuje ten przypadek jako anulowanie.
`resourceId` wskazuje wyłącznie zarejestrowany lokalny zasób, nie dowolną ścieżkę lub URL.
Nie wystawiamy komend pause/resume ani zmiany globalnej głośności urządzenia.

Przed uruchomieniem produkcyjnym trzeba dodać implementacje, walidację IPC i ustawień,
testy cyklu sukces/błąd/anulowanie oraz politykę uprawnień i CSP dla faktycznych integracji.
