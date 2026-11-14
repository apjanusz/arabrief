# AraBrief

Aplikacja desktopowa Windows: Tauri 2, React 19, TypeScript 6, Vite 8 i Tailwind CSS 4.
Docelowo okresowo ścisza muzykę innej aplikacji, odtwarza krótką audycję AI i przywraca
wcześniejszą głośność. Muzyka nigdy nie jest pauzowana.

## Obecny zakres

Minimalny polski ekran ze statusem, wyborem sesji oraz interfejsy modułów. Nie ma jeszcze integracji
RSS, API, syntezy mowy ani działającego harmonogramu. Ustawienia są zapisywane w SQLite,
a klucz API w Windows Credential Manager.
Odczyt sesji, ducking i natywne odtwarzanie WAV/MP3 są aktywne. Dostępny jest test
całego cyklu z muzyką i dwutonowym nagraniem WAV; generowanie mowy pozostaje planem.

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

Nazwy komend i typy danych są w `src/platform/tauri/contracts.ts`. Działają wszystkie
poniższe komendy. Komendy odtwarzacza odrzucają Promise z tekstem błędu; komendy sesji
używają obiektu NativeError.

| Komenda | Argumenty invoke | Wynik |
| --- | --- | --- |
| `audio_list_sessions` | brak | lista sesji (bez sesji AraBrief) |
| `audio_duck` | `{ request: { sessionId } }` | `{ restorationToken, outcome }` po fade-out |
| `audio_restore` | `{ request: { restorationToken } }` | `completed` lub `manual_override` po fade-in |
| `audio_play` | `{ request: { resourceId, musicSessionId?, outputDeviceId? } }` | PlaybackStatus po końcu/zatrzymaniu i przywróceniu muzyki |
| `audio_stop` | brak | null po zgłoszeniu zatrzymania (stan końcowy przez getStatus/event) |
| `audio_get_status` | brak | PlaybackStatus |
| `audio_set_volume` | `{ volume }` | null; głośność lektora 0–1 |

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

## Natywny odtwarzacz i test audycji

Implementacja: `src-tauri/src/player.rs`, rodio 0.23.0, CPAL/WASAPI na Windows.
Włączone są tylko features playback, wav i mp3. API rodio zostało sprawdzone:
https://docs.rs/rodio/0.23.0/rodio/struct.Player.html
https://docs.rs/rodio/0.23.0/rodio/stream/struct.DeviceSinkBuilder.html

Port TypeScript: `src/modules/audio/AudioPlayer.ts`, adapter Tauri:
`src/platform/tauri/player.ts`. Metody: play, stop, getStatus, setVolume.
`audio-playback-status` emituje PlaybackStatus przy zmianach fazy. Fazy: idle,
preparing, ducking, playing, restoring, ended, stopped, error. playbackId rozróżnia
kolejne nagrania. Stan końcowy i Promise play obejmują również sprzątanie audio;
ended oznacza naturalny koniec, stopped anulowanie, error zawiera opis błędu.
UI odczytuje stan co 250 ms, więc nie zależy od momentu podpięcia listenera.

Przyciski testowe:
- „Odtwórz nagranie”: sam odtwarzacz, bez ściszania muzyki.
- „Test audycji z muzyką”: przygotowanie/dekodowanie i otwarcie wyjścia → fade-out
  wybranej sesji → nagranie → faktyczne opróżnienie kolejki Player → fade-in.
- „Zatrzymaj nagranie”: zatrzymuje tylko nagranie; przywraca ściszoną muzykę.
- Suwak steruje wyłącznie mnożnikiem próbek lektora (domyślnie 70%).

Wbudowany resourceId `test.wav` to autorskie dwusekundowe nagranie PCM z dwoma tonami,
generowane lokalnie; nie udaje syntezy mowy. Inne WAV/MP3 można umieścić w
`app_data_dir()/audio` (Windows: katalog danych AraBrief pod APPDATA) i wywołać play
z samą nazwą pliku jako resourceId. Bez ścieżek, URL ani dostępu do dowolnych plików.
Limit pliku: 20 MiB; limit dekodowania: 16 mln próbek. Nagranie jest dekodowane przed
ściszeniem, żeby błąd otwarcia pliku nie wpływał na muzykę. Rodio/Symphonia może
pominąć uszkodzone ramki MP3; nie jest to walidator integralności nagrań.

Domyślnie wyjściem jest bieżące domyślne urządzenie odtwarzania Windows, otwierane
na początku każdego nagrania. Opcja przy testach pozwala użyć deviceId wybranej
sesji muzycznej. Nie ma automatycznego fallbacku na inne głośniki. Odłączenie wyjścia
lub błąd strumienia CPAL kończy odtwarzanie błędem i uruchamia przywracanie muzyki.
Zmiana domyślnego urządzenia będzie uwzględniona przy następnym play.

Nie ma timeoutu długości nagrania: warunek końca to Player.empty(), a 20 ms to jedynie
częstotliwość sprawdzania kolejki, anulowania i błędów. Jest to koniec konsumpcji
źródła przez rodio; sprzęt może mieć niewielką dodatkową latencję bufora. Strumień
wyjściowy pozostaje przy życiu przez przywracanie głośności. Jeden play naraz,
niezależna głośność lektora i własna sesja procesu AraBrief. Cleanup jest natywny,
również po błędzie lub utracie UI. Normalne zamknięcie najpierw zatrzymuje odtwarzacz
oraz czeka na przywrócenie muzyki, a następnie zamyka kontroler duckingu.

Testy bez odsłuchu: dekodowanie WAV i syntetycznych ramek MP3, błędny plik,
kolejność zakończenie/stop/błąd → restore, raportowanie błędów przywracania,
walidacja głośności oraz blokada równoległego play. Ręcznie sprawdź oba przyciski,
zmianę głośności lektora w trakcie odtwarzania, stop podczas fade-out i nagrania,
odłączenie wyjścia oraz zamknięcie aplikacji w trakcie audycji.

## Trwałe ustawienia i bezpieczny klucz OpenAI

Natywne repozytorium `src-tauri/src/settings.rs` używa rusqlite 0.42 z wbudowanym
SQLite (bez instalowania bazy na komputerze użytkownika i bez pluginu SQL w React).
Plik: `app_data_dir()/settings.sqlite3`, zwykle
`%APPDATA%/com.janus.ara-brief/settings.sqlite3` na Windows. Baza powstaje przy starcie.
Nie zawiera klucza API. Migracje w `src-tauri/migrations` są stosowane w transakcji
IMMEDIATE, wersjonowane przez PRAGMA user_version. Nowszy nieobsługiwany schemat,
błąd migracji lub niepoprawne dane przerywają inicjalizację zamiast resetować ustawienia.

| Pole DTO | Domyślna wartość | Walidacja |
| --- | --- | --- |
| preferredAudioSource | null | para id/nazwa, limity 4096/256 bajtów |
| intervalMs | 1800000 (30 min) | 1 min – 24 h |
| duckVolume | 0.15 | mnożnik pierwotnego poziomu, 0–1 |
| fadeOutMs / fadeInMs | 3000 / 3000 | 0–30000 ms |
| narratorVolume | 0.7 | 0–1 |
| broadcastMinDurationMs / broadcastMaxDurationMs | 120000 / 180000 | uporządkowany zakres 30 s – 15 min |
| topics | [] | do 30 unikalnych tematów |
| openAiVoice | alloy | identyfikator; bez weryfikacji przez OpenAI |
| autoStart | false | bool; harmonogram nadal nie jest wdrożony |
| language / newsSourceUrls | pl / [] | preferencje przyszłego modułu wiadomości |

`settings_load()` zwraca kompletny AppSettings; `settings_save({ settings })` zapisuje
pełny, zwalidowany snapshot. Numer wersji DTO (version: 1) jest niezależny od wersji
schematu bazy (obecnie 2). Nieznane pola są odrzucane, więc apiKey nie może zostać
przemycone jako pole ustawień. Tematy i adresy są w osobnych tabelach; zapis całego
snapshotu jest atomowy. Kontrakty i adapter TS: `src/platform/tauri/settings.ts`.

Zapisane głośności i czasy fade są stosowane przy starcie i po settings_save. Trwający
cykl zachowuje własny snapshot konfiguracji. Suwak lektora zmienia głośność na żywo;
przycisk „Zapisz źródło i głośność lektora” utrwala ją wraz z aktualnym wyborem sesji.
Pozostałe pola są dostępne przez repozytorium/IPC; celowo nie ma pełnego formularza.
Identyfikator sesji Windows może wygasnąć po restarcie odtwarzacza. Wówczas preferencja
pozostaje w bazie, ale trzeba ponownie wybrać i zapisać aktualną sesję. Nie dopasowujemy
samoczynnie po PID ani nazwie, żeby nie wybrać innego procesu.

Klucz: `src-tauri/src/credentials.rs`, natywne CredWriteW/CredReadW/CredDeleteW.
Target: `com.janus.ara-brief/openai-api-key`, CRED_TYPE_GENERIC,
CRED_PERSIST_LOCAL_MACHINE (trwałość między logowaniami w magazynie bieżącego
użytkownika Windows; nie oznacza udostępnienia wszystkim użytkownikom komputera).

- `credentials_save()` otwiera natywne okno Windows; klucz wpisz w polu hasła.
  Zwraca true po zapisie, false po anulowaniu. Anulowanie zachowuje poprzedni klucz.
- `credentials_check()` zwraca wyłącznie informację o obecności niepustego wpisu.
  Nie sprawdza ważności, uprawnień ani środków na koncie OpenAI.
- `credentials_delete()` usuwa wpis; brak wpisu również oznacza sukces.

Klucz nie przechodzi przez DOM, stan React ani argumenty/wynik IPC. Bufory Rust są
czyszczone przez zeroize, a bufor pobrany z Windows jest czyszczony przed CredFree.
Nie ma komendy zwracającej klucz, logowania wartości ani zapisu do pliku/.env/SQLite.
Przyszły klient OpenAI musi działać w Rust i korzystać z tego magazynu; żadnych
zapytań do OpenAI nie dodano. Na innych systemach operacje klucza zwracają błąd,
bez fallbacku do plaintext.

Testy repozytorium obejmują domyślne wartości, zapis/odczyt, ponowne otwarcie pliku,
odrzucanie niepoprawnych ustawień, migrację v1 → v2, rollback nieudanej migracji
i odrzucenie nowszego schematu. Nie używają rzeczywistego klucza. Ręczna weryfikacja:
zapisz preferencje, uruchom aplikację ponownie; zapisz klucz w natywnym oknie,
sprawdź obecność, anuluj ponowny zapis i usuń wpis. Sprawdzenie jest lokalne.

Dokumentacja:
https://docs.rs/rusqlite/0.42.0/rusqlite/struct.Connection.html
https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew
