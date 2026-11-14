# AraBrief

Aplikacja desktopowa Windows: Tauri 2, React 19, TypeScript 6, Vite 8 i Tailwind CSS 4.
Docelowo okresowo ścisza muzykę innej aplikacji, odtwarza krótką audycję AI i przywraca
wcześniejszą głośność. Muzyka nigdy nie jest pauzowana.

## Obecny zakres

Minimalny polski ekran ze statusem, wyborem sesji oraz interfejsy modułów. Nie ma jeszcze integracji
RSS, API, syntezy mowy, zapisu ustawień ani działającego harmonogramu.
Odczyt sesji i ręczny ducking są aktywne; odtwarzanie głosu pozostaje planem.

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

## Kontrakt natywny

Nazwy komend i typy danych są w `src/platform/tauri/contracts.ts`. Działają audio_list_sessions, audio_duck i audio_restore. Komendy odtwarzania nie są zarejestrowane.

| Komenda | Argumenty invoke | Wynik |
| --- | --- | --- |
| `audio_list_sessions` | brak | lista sesji (bez sesji AraBrief) |
| `audio_duck` | `{ request: { sessionId } }` | `{ restorationToken, outcome }` po fade-out |
| `audio_restore` | `{ request: { restorationToken } }` | `completed` lub `manual_override` po fade-in |
| `audio_play` | `{ request: { resourceId } }` | null po zakończeniu odtwarzania |
| `audio_stop` | brak | null po zatrzymaniu głosu |

Rust ma używać serde camelCase dla DTO i `Result<T, NativeError>`; błędy odrzucają
Promise z `{ code, message }`. Kontrakty TS nie walidują danych w czasie wykonania.
Natywna strona musi walidować sesje, skończoną głośność w [0,1] i identyfikatory zasobów.
Token przywrócenia jest nieprzezroczysty: snapshot pozostaje w Rust. Ściszenie nigdy nie
zwiększa głośności; po błędzie podejmowana jest próba przywrócenia snapshotu.
Przywracanie jest idempotentne. Zakończona sesja nie może blokować przywracania pozostałych.
Rust musi również zapewnić odzyskiwanie głośności przy zamknięciu/awarii warstwy UI.
`audio_stop` kończy aktywne `audio_play`; scheduler traktuje ten przypadek jako anulowanie.
`resourceId` wskazuje wyłącznie zarejestrowany lokalny zasób, nie dowolną ścieżkę lub URL.
Nie wystawiamy komend pause/resume ani zmiany globalnej głośności urządzenia.

Przed uruchomieniem produkcyjnym trzeba dodać implementacje, walidację IPC i ustawień,
testy cyklu sukces/błąd/anulowanie oraz politykę uprawnień i CSP dla faktycznych integracji.

## Natywny odczyt sesji audio

`audio_list_sessions` jest już zarejestrowaną komendą Tauri. Adapter React w
`src/platform/tauri/audio.ts` wywołuje ją i przekazuje wynik do AudioSessionPicker.
W trybie przeglądarkowym pojawia się informacja o wymaganej aplikacji desktopowej.
Wybór jest lokalnym stanem komponentu; dopiero przyciski duckingu zmieniają głośność.

Implementacja: `src-tauri/src/audio/windows.rs`, windows-rs 0.62.2. Enumeruje wszystkie
aktywne urządzenia renderujące (nie tylko domyślne) i ich sesje przez
IAudioSessionManager2 / IAudioSessionEnumerator. Zwraca id instancji z prefiksem
urządzenia, sessionIdentifier, deviceId, pid, opcjonalny processName, displayName,
volume [0,1] i state (active/inactive). Sesje nieaktywne są dostępne do wyboru;
wygasłe, dźwięki systemowe i własny proces są pomijane. Urządzenia odłączone lub
niedostępne w trakcie odczytu są pomijane (błąd trafia na stderr); wynik może być częściowy.
Błędy inicjalizacji COM lub enumeracji urządzeń trafiają do UI jako NativeError.

COM jest inicjalizowany jako MTA w spawn_blocking i zwalniany na tym samym wątku.
Żadne wskaźniki COM ani uchwyty procesów nie trafiają do UI lub cache. Napisy COM
są zwalniane przez CoTaskMemFree, uchwyty procesów przez CloseHandle. Odczyt nazwy
używa PROCESS_QUERY_LIMITED_INFORMATION; proces chroniony lub zakończony daje null,
bez paniki. PID jest wyłącznie metadanymi, a identyfikatorem wyboru jest instancja
sesji na urządzeniu. Migawka może zdezaktualizować się natychmiast po odczycie;
odświeżenie usuwa wybór, jeśli sesja zniknęła. Zmiana urządzenia lub restart odtwarzacza
wymaga ponownego wyboru. Nie ma monitorowania zdarzeń ani trwałej identyfikacji procesu.

Ręczna weryfikacja: `npm run tauri dev`, włącz muzykę, odśwież listę i wybierz sesję;
sprawdź PID, nazwę i głośność, zakończ odtwarzacz i ponownie odśwież. Przy dwóch
aktywnych wyjściach sprawdź sesje na obu urządzeniach. Brak zmiany głośności jest zamierzony.
Test odczytu i serializacji: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --nocapture`.

## Ręczny ducking

`Ścisz muzykę` używa wyłącznie ISimpleAudioVolume wybranej instancji sesji.
Fade-out i fade-in trwają po 3 sekundy (kroki co około 20 ms, postęp liczony zegarem).
Cel to 15% poziomu początkowego: np. 80% → 12%, a nie bezwzględne 15%.
`Przywróć głośność` odtwarza początkowy poziom i zwalnia cykl. Brak pauzy/wznowienia,
zmiany globalnej głośności, stanu mute lub głośności innych sesji.

Jeden dedykowany wątek MTA przechowuje snapshot, uchwyty COM i subskrypcję
IAudioSessionEvents. Obsługuje jeden cykl naraz; ponowne ducking jest odrzucane, więc
nie da się zastąpić snapshotu już ściszonym poziomem. Restore jest idempotentne po
zakończeniu cyklu. Pusty restorationToken oznacza przywrócenie bieżącego cyklu
(przycisk awaryjny także po błędzie komendy lub przeładowaniu UI).

Własne zapisy mają EventContext GUID. Zewnętrzne zmiany poziomu, mute lub kanałów
powodują oddanie kontroli użytkownikowi: fade kończy się wynikiem manual_override,
a późniejsze restore nie nadpisuje jego poziomu. Dodatkowo przed każdym zapisem
odczytywany poziom jest porównywany z ostatnim własnym zapisem. Powiadomienia Windows
są asynchroniczne: odczyt i zapis nie są atomową operacją względem miksera systemowego.
Po oddaniu kontroli należy zakończyć cykl przyciskiem przywracania przed następnym duckingiem.

Wygaśnięcie sesji lub odłączenie urządzenia kończy operację błędem. Nie próbujemy
przenosić snapshotu na inną sesję, PID lub nowe urządzenie. Gdy to samo urządzenie
zmieni status domyślnego, nadal działamy wyłącznie na pierwotnej sesji. Jeśli sesja
przeniesie się na inne wyjście, trzeba ją ponownie wybrać. Po błędzie snapshot jest
zachowany, jeżeli automatyczna próba przywrócenia nie powiodła się.

Normalne ExitRequested jest wstrzymywane, a wątek audio kończy trwający fade i
wykonuje restore przed wyjściem. Zmiany użytkownika nadal mają pierwszeństwo.
Niedostępnej lub wygasłej sesji nie da się przywrócić; błąd zamknięcia jest raportowany
na stderr. Wymuszone zabicie procesu/awaria systemu nie wykonuje tego mechanizmu.

Testy logiki fade używają fikcyjnego adaptera i nie zmieniają realnej głośności.
Ręcznie sprawdź: fade 80% → 12% → 80%, ręczną zmianę w mikserze w trakcie fade i po
ściszeniu, zamknięcie okna podczas fade, zakończenie odtwarzacza i odłączenie urządzenia.

Referencje API:
- https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-isimpleaudiovolume-setmastervolume
- https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IAudioSessionControl2.html
- https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nn-audiopolicy-iaudiosessionmanager2
- https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-enumaudioendpoints
