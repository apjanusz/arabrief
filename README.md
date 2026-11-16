# AraBrief

Aplikacja desktopowa Windows: Tauri 2, React 19, TypeScript 6, Vite 8 i Tailwind CSS 4.
Po uruchomieniu harmonogramu okresowo ścisza muzykę innej aplikacji, odtwarza krótką audycję AI i przywraca
wcześniejszą głośność. Muzyka nigdy nie jest pauzowana.

## Obecny zakres

Polski interfejs z harmonogramem, wyborem sesji, zainteresowaniami i ustawieniami.
Synteza mowy przez Realtime, Google News RSS i generowanie scenariuszy
przez OpenAI Responses API działają natywnie. Ustawienia są zapisywane w SQLite,
a klucz API w Windows Credential Manager.
Odczyt sesji, ducking i natywne odtwarzanie WAV/MP3 są aktywne. Dostępny jest test
całego cyklu z muzyką i dwutonowym nagraniem WAV.

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
Testy logiki uruchamiają `node:test`; build sprawdza TypeScript.

Tailwind jest podłączony przez `@tailwindcss/vite` i `@import "tailwindcss"` w App.css,
zgodnie z https://tailwindcss.com/docs/installation/using-vite. Nie wymaga PostCSS ani
osobnego pliku konfiguracji Tailwind.

## Struktura i odpowiedzialności

Pomiary wydajności i porównanie kolejki pobierania opisuje
[docs/performance.md](docs/performance.md). W terminalu `npm run tauri dev` wpisy
`[perf]` podają nazwę operacji, czas monotoniczny w ms i wynik (`ok`, `cache_hit`
lub `error_or_cancelled`). Logowane są etapy, a nie fragmenty PCM czy treść promptów.

| Lokalizacja | Odpowiedzialność |
| --- | --- |
| `src/App.tsx`, `src/App.css` | Ekrany desktopowe i style Tailwind |
| `src/domain/models.ts` | Dane wiadomości, scenariusza, audio, ustawień i stanu |
| `src/modules/audio/AudioController.ts` | Port do natywnego Windows Audio: sesje, ściszenie, odtwarzanie i przywrócenie |
| `src/modules/news/NewsProvider.ts` | Pobieranie i normalizacja wiadomości |
| `src/modules/content/ContentGenerator.ts` | Generowanie scenariusza z zachowaniem źródeł |
| `src/modules/speech/SpeechProvider.ts` | Synteza Realtime, cache lokalnego WAV i anulowanie |
| `src/modules/scheduler/BroadcastScheduler.ts` | Interwały, orkiestracja, anulowanie, stan; jedna audycja naraz |
| `src/modules/scheduler/AutomaticBroadcastScheduler.ts` | Implementacja harmonogramu, zegar monotoniczny, obsługa wybudzenia i błędów |
| `src/modules/settings/SettingsRepository.ts` | Walidacja, wersjonowanie i zapis ustawień bez sekretów |
| `src/platform/tauri/contracts.ts` | Typy komend IPC, niezależne od UI |
| `src-tauri/src/lib.rs` | Punkt kompozycji adapterów natywnych |

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
Limit pliku: 64 MiB; limit dekodowania: 32 mln próbek. Nagranie jest dekodowane przed
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

Obsługiwany schemat SQLite ma wersję 8. Zachowane migracje 006–007 odpowiadają
za dodatkowe tabele materiałów artykułów; pozostają w historii schematu także
po cofnięciu kodu korzystającego z tych tabel. Baza w wersji 7 przechodzi migrację 008 bez
ponownego wykonywania migracji i bez obniżania jej numeru. Git stash ani cofnięcie
kodu nie cofają pliku bazy w AppData. Nie usuwaj bazy w celu naprawy niezgodności:
uruchom kod obsługujący jej schemat. Nieznane przyszłe wersje nadal są odrzucane,
z informacją o numerze bazy i maksymalnej obsługiwanej wersji.

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
| autoStart | false | bool; zapisane pole nie uruchamia aplikacji samoczynnie, Start naciska użytkownik |
| language / newsSourceUrls | pl / [] | preferencje przyszłego modułu wiadomości |

`settings_load()` zwraca kompletny AppSettings; `settings_save({ settings })` zapisuje
pełny, zwalidowany snapshot. Numer wersji DTO (version: 1) jest niezależny od wersji
schematu bazy (obecnie 8). Nieznane pola są odrzucane, więc apiKey nie może zostać
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
sprawdź klucz online bez generacji, anuluj ponowny zapis i usuń wpis. Lokalna kontrola obecności pozostaje osobną komendą.

Dokumentacja:
https://docs.rs/rusqlite/0.42.0/rusqlite/struct.Connection.html
https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew

## Google News RSS / NewsProvider

Porty: `src-tauri/src/news.rs::NewsProvider` i `src/modules/news/NewsProvider.ts`.
Pierwszy adapter to GoogleNews; normalizuje metadane RSS do NewsItem. Warstwa HTTP
jest oddzielona od repozytorium historii i widoku. Nowy dostawca może implementować
fetch_topic, zachowując provider i pochodzenie danych; serwis można rozszerzyć o jego
wybór i osobny klucz limitów odpytywania. Nie ma integracji z OpenAI ani pobierania
pełnych stron wydawców.

Adres zapytania jest budowany przez Url.query_pairs_mut (bez sklejania danych użytkownika):
`https://news.google.com/rss/search?q=…&hl=pl&gl=PL&ceid=PL:pl`.
To preferencja polskiej edycji, nie gwarancja języka każdego wpisu. RSS nie jest
wersjonowanym kontraktem API Google. Pobierane są wyłącznie kanały wyszukiwania RSS;
nie dekodujemy linków pośrednich Google, nie obchodzimy paywalli ani nie wysyłamy
zapytań do adresów artykułów. TLS na Windows korzysta z systemowej weryfikacji certyfikatów.

Komendy Tauri:
- `news_fetch({ topics: string[] })` → `{ items, topics, fetchedTopics }`.
  topics wyniku zawiera topic, nextFetchAt (Unix seconds) i error. fetchedTopics
  oznacza tematy, dla których podjęto próbę HTTP w tym wywołaniu. Przy limicie
  lub błędzie zwracane są dostępne wcześniejsze, niewykorzystane wpisy z SQLite.
- `news_mark_used({ articleIds, broadcastId })` → null. Wywołuj dopiero dla artykułów
  faktycznie wykorzystanych w audycji. Zapis transakcyjny i idempotentny dla pary
  artykuł/audycja; nieznany identyfikator odrzuca cały zapis. Nie jest wywoływany
  przez podgląd diagnostyczny ani test dwutonowego WAV.

NewsItem zachowuje tytuł, oryginalny URL, nazwę i URL źródła (jeżeli podane), GUID,
URL kanału, datę pobrania oraz oryginalną datę publikacji. Poprawna data publikacji
jest dodatkowo normalizowana do UTC; brak lub błędna data pozostaje null i trafia
na koniec sortowania. Brak źródła i opisu pozostaje null. descriptionHtml to surowy
opis RSS, nigdy pełna treść ani streszczenie przygotowane przez model. Google często
podaje tam tylko nagłówek i link. Diagnostyka renderuje go jako tekst, bez HTML.
Nie wolno traktować takich danych jako wiedzy o nieudostępnionej treści artykułu.

Migracja `003_news_history.sql` rozbudowuje tę samą bazę settings.sqlite3 o artykuły,
powiązania z tematami, aliasy URL/GUID z metadanymi, historię użycia i stan odpytywania.
Duplikaty są rozpoznawane po kanonicznym URL (bez fragmentu i znanych parametrów
śledzących), GUID dostawcy oraz dokładnym znormalizowanym tytule + źródle + dniu UTC.
Nie jest to semantyczna deduplikacja: różne relacje o jednym wydarzeniu mogą pozostać
osobnymi wynikami. Każdy alias zachowuje metadane RSS; główny wpis zachowuje pierwszy
snapshot. Zapisane użycie wyklucza artykuł ze wszystkich tematów również po restarcie.
Z każdej listy tematu zwracanych jest do 100 ostatnio widzianych niewykorzystanych
artykułów; wspólna lista jest sortowana malejąco według daty publikacji.

Limity i błędy:
- 1–30 tematów, do 160 bajtów każdy; duplikaty tematów bez rozróżniania wielkości liter.
- Do czterech równoległych żądań, jedna partia odświeżania naraz w procesie.
- Timeout całego żądania 12 s, połączenia 5 s; maksymalnie 2 MiB RSS i 100 wpisów z kanału.
- Minimum 5 minut na temat oraz minuta pomiędzy partiami nowych zapytań; limity
  są rezerwowane transakcyjnie w SQLite przed HTTP, także dla nieudanych prób.
- HTTP 429/503 ustawia cooldown dostawcy według Retry-After (domyślnie 5 min,
  minimum 1 min, maksimum doba). Po takim błędzie nie startują następne grupy żądań.
- Błędy tematów są zapisywane i pokazywane osobno, więc jeden niedostępny kanał
  nie usuwa poprawnych wyników z pozostałych. Nie ma automatycznych retry ani
  nieograniczonych przekierowań; przekierowanie HTTP jest raportowane jako błąd.

Widok „Wiadomości — diagnostyka RSS” przyjmuje po jednym temacie na linię, zapisuje
listę do preferencji i pozwala ręcznie pobrać wiadomości. Pokazuje pochodzenie,
metadane, opis RSS, błędy, ograniczenia odpytywania i datę pobrania danych z cache.

Testy obejmują parsowanie, brakujące pola, odrzucanie niebezpiecznych URL, sortowanie,
deduplikację między tematami, aliasy, wykluczanie użytych artykułów, atomowość historii,
limity i trwałość po ponownym otwarciu SQLite. Test rzeczywistego kanału jest opcjonalny:
`cargo test --manifest-path src-tauri/Cargo.toml --lib news::tests::live_google_news -- --ignored --nocapture`.
Nie zapisuje danych w produkcyjnej bazie i nie otwiera artykułów.

Dokumentacja bibliotek:
https://docs.rs/rss/2.1.2/rss/struct.Channel.html
https://docs.rs/reqwest/0.13.5/reqwest/blocking/struct.ClientBuilder.html

## ContentGenerator / OpenAI Responses API

Moduł Rust: `src-tauri/src/content.rs`. Prompt lektora: `src-tauri/src/arabrief_prompt.txt`.
Port TS: `src/modules/content/ContentGenerator.ts`, adapter Tauri:
`src/platform/tauri/content.ts`. Nie dodano SDK ani nowych zależności. HTTP korzysta
z istniejącego reqwest, a klucz pobierany jest przez natywną funkcję credentials
(nie komendę IPC). Authorization ma oznaczenie sensitive; klucz i bufor Bearer są
Zeroizing. Klucz nie jest częścią promptu, JSON requestu, SQLite, logów ani odpowiedzi UI.

`content_generate({ articleIds })` przyjmuje od 1 do 8 identyfikatorów NewsProvider.
Powtórzone ID są scalane; brakujące, zarezerwowane i uszkodzone wpisy są pomijane.
Nie przyjmuje dowolnych tekstów, URL-i ani klucza od webview. Rust pobiera snapshoty
RSS z bazy i sprawdza, czy nie były wcześniej użyte w audycji ani zapisanym scenariuszu.
Opis każdego źródła wysyłany do modelu ma limit 4000 znaków; pełne metadane nadal są
w historii RSS. Model tekstowy jest zapisany w ustawieniu textModel (domyślnie
`gpt-5-mini`), konfigurowalny w polu nad przyciskiem. Musi obsługiwać Responses API
oraz Structured Outputs. Ustawienie openAiVoice jest niezależne i nadal nie jest używane.

Zapytanie: POST https://api.openai.com/v1/responses, instructions + input,
text.format=json_schema, strict=true, store=false, max_output_tokens=6000.
Nie ma narzędzi, wyszukiwania internetu, previous_response_id ani dowolnego endpointu.
Timeout całości 90 s, połączenia 10 s, limit odpowiedzi 512 KiB. Bez automatycznych
ponowień płatnych żądań. HTTP 401/403, 429, błędy modelu, odmowa i nieczytelna koperta
JSON kończą próbę; gotowa niewykorzystana audycja pozostaje dostępna jako fallback.
Kompletny JSON odpowiedzi incomplete może dostarczyć poprawne segmenty. Wadliwy segment
nie niszczy poprawnych sąsiadów. Surowe odpowiedzi błędów API nie trafiają do UI/logów.
Jedno generowanie naraz w procesie.

Scenariusz domyślnie celuje w 280–420 słów (2–3 minuty przy 140 słowach/min).
Zakres pochodzi z broadcastMinDurationMs/broadcastMaxDurationMs. Przy skąpych danych
powinien być krótszy, oznaczony insufficient_material i opisany w limitations.
Nie wyciągamy treści artykułu z samego nagłówka i nie uzupełniamy długości faktami
z wiedzy modelu. estimatedDurationMs to oszacowanie z liczby słów, nie pomiar TTS.
Przy przekroczeniu 120% górnego celu usuwane są całe nadmiarowe wiadomości.

Ustrukturyzowany wynik BroadcastScript zawiera id, createdAt (UTC), model, title,
text, language=pl, topics, estimatedDurationMs, docelowy zakres czasu, draft i sources.
draft ma title, segments (topic, text, evidence: source_id + quote), limitations
oraz insufficient_material. Wewnętrzne pola draft/evidence zachowują nazwy schema
OpenAI w snake_case; DTO aplikacji i snapshoty NewsItem używają camelCase.
URL-e i metadane sources pochodzą wyłącznie z bazy, nie z odpowiedzi modelu.

Walidacja sprawdza identyfikatory z przekazanego zbioru i dosłowne cytaty obecne
w dostarczonym tytule/opisie. Usuwa pojedyncze wiadomości z nieznanym źródłem,
zmyślonym cytatem lub powtórzoną narracją. Sam współdzielony cytat nie jest powodem
odrzucenia; uwzględniamy też poprzedni tekst i obecność nowych dowodów. Wiadomości
porządkowane są lokalnie od najnowszej publikacji; nie ma ponownego wywołania modelu.
Jedna poprawna wiadomość wystarcza do krótszej audycji. Prompt nakazuje przypisać każdą istotną informację do
potwierdzającego cytatu i traktować RSS jako niezaufane dane, nigdy polecenia.
Istnienie cytatu nie dowodzi automatycznie, że każda parafraza modelu z niego wynika:
to walidacja pochodzenia, nie niezależny weryfikator prawdziwości lub semantyki.
W podglądzie można sprawdzić cytaty dla każdego segmentu przed przyszłym odtworzeniem.

Migracja `004_content_generation.sql` dodaje text_model oraz content_broadcasts,
content_sources i content_evidence. Tekst, data, tematy, model, ograniczenia i snapshoty
wykorzystanych źródeł są zapisywane w jednej transakcji. Nie zapisujemy chain-of-thought.
Ponowne użycie tych samych artykułów jest blokowane przez historię całej
bazy; pięć ostatnich scenariuszy jest dodatkowo przekazywanych modelowi jako kontekst
przeciw powtarzaniu informacji innymi słowami. To ogranicza, ale nie gwarantuje wykrycia
semantycznych powtórzeń tej samej informacji w innych źródłach, zwłaszcza starszych.

Zapisany scenariusz testowy nie jest wyemitowaną audycją: nie dodaje wpisu news_usage,
nie uruchamia odtwarzacza, duckingu ani TTS. Jest jednak konserwatywnie wykluczany
z materiałów następnych scenariuszy. news_fetch pomija źródła już związane z zapisanym
scenariuszem; po wyczerpaniu dostępnych materiałów należy pobrać nowe wiadomości.

Test ręczny: zapisz klucz w Windows Credential Manager, pobierz RSS dla kilku tematów,
sprawdź materiały i model, kliknij „Wygeneruj testową audycję”. Przycisk wysyła do
OpenAI materiały RSS i kontekst poprzednich scenariuszy oraz może naliczyć koszt API.
UI wyświetla tekst, źródła, cytaty, ograniczenia i orientacyjny czas. TTS i harmonogram
opisano poniżej. Testy automatyczne używają fikcyjnego generatora i
odpowiedzi API, bez rzeczywistego klucza, sieci OpenAI lub kosztów. Sprawdzają kontrakt
Responses, błędy/refusal/incomplete, źródła, cytaty, powtórzenia i atomowość zapisu.

Referencje API użyte podczas implementacji:
https://developers.openai.com/api/reference/resources/responses/methods/create
https://developers.openai.com/api/docs/models/gpt-5-mini

## SpeechProvider / Realtime → lokalny WAV

Implementacja: `src-tauri/src/speech/{mod.rs,realtime.rs,format.rs}`. Port TS:
`src/modules/speech/SpeechProvider.ts`, adapter `src/platform/tauri/speech.ts`.
Stały model `gpt-realtime-2.1-mini`, aktualny Realtime API przez natywny WebSocket
`wss://api.openai.com/v1/realtime?model=gpt-realtime-2.1-mini`. Brak fallbacku na inny
model i brak użycia endpointu audio/speech. Klucz pobiera tylko Rust z Windows
Credential Manager i wysyła jako wrażliwy nagłówek Authorization po TLS; nie trafia
do React, URL-a, SQLite, logów ani tekstu wysyłanego modelowi.

Przycisk „Wygeneruj audio” pojawia się pod przygotowanym scenariuszem. Można wybrać
jeden z wbudowanych głosów Realtime (alloy, ash, ballad, coral, echo, sage, shimmer,
verse, marin, cedar); wybór jest zapisywany w openAiVoice. Generowanie nowego pliku
jest płatne. „Anuluj generowanie” zatrzymuje bieżące zadanie, „Odtwórz audio” korzysta
z natywnego odtwarzacza AraBrief, a „Zatrzymaj nagranie” zatrzymuje tylko to nagranie.
Głośność pozostaje niezależną głośnością lektora istniejącego AudioPlayer.
Nie uruchamia to automatycznego harmonogramu ani samo nie oznacza emisji audycji.

Komendy:
- `speech_generate({ broadcastId, voice, jobId })` → SpeechAsset. Rust czyta tekst
  zapisanego scenariusza; webview nie podaje dowolnego tekstu ani ścieżki wyjściowej.
  voice może być null (wtedy pobierany jest zapisany wybór). jobId to unikatowy UUID
  zadania generowania, nie klucz API. Jedno zadanie syntezy naraz w aplikacji.
- `speech_cancel({ jobId })` → bool, zgłasza anulowanie tylko wskazanego zadania.
  Obsłużone jest również zgłoszenie anulowania tuż przed rozpoczęciem zadania.

Po session.created wysyłane jest session.update z output_modalities=[audio],
formatem audio/pcm, 24000 Hz, wybranym głosem i turn_detection=null. Kod czeka na
session.updated i weryfikuje głos/format. Nie ma mikrofonu, wejściowego audio,
input_audio_buffer, VAD, WebRTC, narzędzi ani dialogu z użytkownikiem. Każda
response.create jest out-of-band (conversation=none) i zawiera wyłącznie tekst
segmentu oraz stałe polskie instrukcje dokładnego, spokojnego odczytu radiowego.

Skrypt jest dzielony przed wysłaniem na segmenty do 800 znaków, preferując akapity,
potem granice zdań i słów. Tekst nie jest streszczany. Jedna odpowiedź na segment,
max_output_tokens=4096, bez dodawania wcześniejszych odpowiedzi do kontekstu.
Fragmenty response.output_audio.delta są dekodowane z base64 i składane w kolejności
zdarzeń WebSocket. Sprawdzane są response_id, item_id i indeksy treści; duplikaty
event_id są ignorowane. Nie zakładamy, że pojedynczy fragment kończy pełną próbkę:
wyrównanie do 16 bitów jest wymagane dopiero po złożeniu całego segmentu.
response.output_audio.done nie oznacza sukcesu całego zadania: wymagane jest także
response.done ze statusem completed, niepuste PCM oraz końcowa transkrypcja.

response.output_audio_transcript.done jest porównywane ze skryptem każdego segmentu,
a na końcu z całym scenariuszem. Porównanie ignoruje wielkość liter, interpunkcję
i białe znaki; zachowuje słowa, diakrytykę i cyfry. Różnice, dodatkowe powitanie,
pominięcie tekstu lub brak transkrypcji odrzucają nagranie, bez próby automatycznej
naprawy. Zapis cyfr słownie może wywołać konserwatywne odrzucenie. Metadana
verification=realtime_transcript_normalized_exact oznacza zgodność tekstu transkrypcji
modelu, NIE niezależny ASR nagranego WAV ani gwarancję każdej wypowiedzianej głoski.
Nagranie należy odsłuchać przed przyszłą emisją. UI jasno oznacza głos jako AI.

WAV to jeden nagłówek RIFF/WAVE i połączone PCM16 little-endian, mono, 24 kHz.
Długości RIFF/data i durationMs wynikają z faktycznie otrzymanej liczby bajtów,
a nie z czasu oczekiwania. Limit całego audio to 15 minut i 60 segmentów; limit
pojedynczego segmentu 8 MiB. Plik powstaje dopiero po pozytywnej weryfikacji całości.
W katalogu app_data_dir()/audio zapisujemy plik .part, synchronizujemy go i publikujemy
jako speech-{SHA256}.wav przez dowiązanie twarde w tym samym katalogu (bez nadpisywania
istniejącego pliku, wymaga obsługującego je systemu plików, np. NTFS). Po zapisie
SQLite usuwany jest .part. Opublikowany, kompletny WAV pozostaje po późniejszym błędzie
lub anulowaniu. Towarzyszy mu dziennik .wav.json z hashem i zweryfikowaną transkrypcją.
Po awarii zapisu SQLite można lokalnie odbudować indeks po ponownej kontroli formatu,
hasha i transkrypcji, bez wywołania API. Osierocony plik bez poprawnego dziennika
nie jest uznawany za bezpieczne nagranie.

Migracja 005_speech_assets.sql zapisuje hash cache, identyfikator scenariusza,
model, głos, lokalną ścieżkę i resourceId, hash pliku, datę, rzeczywistą długość,
format, liczbę segmentów, transkrypcję oraz wynik weryfikacji. Cache SHA256 zależy
od dokładnego tekstu, głosu, modelu, formatu, instrukcji i wersji segmentacji.
Trafienie sprawdza hash pliku i transkrypcję, działa bez klucza/API i po restarcie.
Uszkodzony/brakujący plik można odbudować lokalnie, jeśli zachowały się wszystkie
zweryfikowane części PCM; wadliwy WAV odkładany jest do pliku .corrupt-*. W przeciwnym
razie nie odtwarzamy go i nie uruchamiamy automatycznie płatnej generacji całego WAV.
Zmiana głosu tworzy inny cache.

Połączenie i handshake mają limit 15 s, zapis 10 s, odpowiedź segmentu 120 s,
a całe zadanie 15 min. Anulowanie wysyła response.cancel, gdy połączenie już istnieje,
i zamyka WebSocket. Całkowity timeout zrywa połączenie. Po błędzie nie jest wysyłana
nowa response.create, nie ma reconnectów, zmiany modelu ani płatnych retry.
Normalne zamknięcie aplikacji anuluje syntezę i czeka na sprzątanie. Opłaty za już
wygenerowaną część mogą pozostać. Każda kompletna część z poprawną transkrypcją jest
cache'owana w SQLite przez 24 godziny (hash PCM, tekst, głos, model i format).
Następna próba lub cykl harmonogramu wznawia zapisany scenariusz, generując tylko
brakujące części. Nie ma serii automatycznych płatnych retry w jednym zadaniu.

Testy bez klucza i kosztów obejmują segmentację Unicode, składanie PCM/WAV, zdarzenia
Realtime, incomplete i obce identyfikatory, dokładność transkrypcji, cache po restarcie,
błędy SQLite, brak ponowień, anulowanie przed i w trakcie generowania oraz zamknięcie.
Płatny test ręczny: wygeneruj krótki scenariusz, wybierz głos, wygeneruj i odsłuchaj WAV,
porównaj transkrypcję, ponownie kliknij generowanie (cache), a osobno sprawdź anulowanie.

Dokumentacja API sprawdzona podczas implementacji:
https://developers.openai.com/api/docs/models/gpt-realtime-2.1-mini
https://developers.openai.com/api/reference/resources/realtime/client-events
https://developers.openai.com/api/reference/resources/realtime/server-events

## Diagnostyka OpenAI i rzeczywisty test klucza

`src-tauri/src/openai_errors.rs` klasyfikuje error.code/error.type dla Responses API,
błędów handshake Realtime i zdarzeń error Realtime. UI dostaje opis po polsku,
konkretny następny krok i bezpieczne metadane: HTTP status, rozpoznany kod/typ,
request-id oraz liczbowe Retry-After (gdy występują). Osobno rozróżniane są:
wyczerpane kredyty, limit wydatków organizacji/projektu, przyznany limit wykorzystania,
insufficient_quota bez rozstrzygniętej przyczyny, RPM/TPM, slow_down, niepoprawny klucz,
uprawnienia, brak modelu i błąd usługi. Nie ma automatycznych płatnych retry.

Surowe error.message ani całe odpowiedzi błędów nie trafiają do UI/logów, ponieważ
OpenAI może umieścić w komunikacie 401 fragment odrzuconego klucza. Kody i typy
pochodzą z allowlisty; nierozpoznany 429 pozostaje nierozstrzygnięty, bez zgadywania.

Przycisk „Sprawdź klucz” wywołuje teraz `credentials_validate()`, a nie samą kontrolę
obecności. Komenda odczytuje klucz w Rust i wysyła pojedyncze GET /v1/models przez
TLS, z timeoutem 15 s i bez przekierowań. Nie generuje tekstu ani audio. Wynik
KeyValidation zawiera status, opis, nazwę skonfigurowanego modelu tekstowego i dwie
opcjonalne informacje o widoczności modelu tekstowego oraz gpt-realtime-2.1-mini.
Nie zawiera klucza, jego prefiksu ani pełnej listy modeli. HTTP 200 potwierdza, że
endpoint modeli przyjął klucz; NIE sprawdza salda, możliwości zapłacenia za generację,
RPM/TPM ani uprawnień zapisu Responses/Realtime. Klucz ograniczony do innych endpointów
może otrzymać 403: wynik jest wtedy inconclusive, a nie automatycznie „nieważny klucz”.
Dotychczasowe credentials_check() nadal jest lokalną kontrolą obecności wpisu.

Ręczny test diagnostyczny tego samego klucza i aktualnie zapisanego modelu, bez
modyfikowania bazy i bez generowania treści:
`cargo test --manifest-path src-tauri/Cargo.toml --lib credentials::validation_tests::live_saved_key_metadata -- --ignored --nocapture`.
Test drukuje tylko bezpieczny raport KeyValidation, nigdy wartość klucza.

Kody sprawdzono w dokumentacji:
https://developers.openai.com/api/docs/guides/error-codes

## Interfejs desktopowy

Powłoka w `src/app` ma trzy widoki: Twoje studio, Zainteresowania i Ustawienia.
Ciemny motyw korzysta z oryginalnego logo w `src/assets/brand`: granatu, czerwieni,
żółtego, niebieskiego i zieleni. Tokeny CSS są w `src/styles/tokens.css`, a opis
kierunku projektowego i komponentów w [docs/ui-design.md](docs/ui-design.md).
Używamy Tailwind, systemowej czcionki Segoe UI i SVG, bez nowych zależności.
Domyślne okno ma 1200 × 820 px; przy mniejszych szerokościach układ zmienia się
na jedną kolumnę, a nawigacja boczna na górną. Formularze pozostają zamontowane
przy zmianie widoku; zapisane ustawienia synchronizują ich stan. Pasek zapisu
przykleja się do dołu tylko przy niezapisanych zmianach.
Bez zewnętrznych fontów, bibliotek komponentów, routera i animacji. Nawigacja przechodzi
w poziome przyciski w wąskim oknie. Formularze mają etykiety, widoczny focus, obsługę
klawiatury i statusy; dane nie są zastępowane przykładowymi wartościami przy braku IPC.

`useAraBrief` jest punktem kompozycji istniejących adapterów i stanu UI. Nie wywołuje
Windows API; połączenie z sesjami, odtwarzanie, ducking, SQLite i poświadczenia obsługują
istniejące serwisy. Ustawienia zapisują pełny snapshot AppSettings, zachowując pola
nieedytowane w formularzu. Zainteresowania są zapisywane od razu po dodaniu/usunięciu,
pozostałe ustawienia dopiero po „Zapisz ustawienia”. Źródło muzyki wybiera się z realnej
listy sesji. Klucz nadal wpisuje się wyłącznie w natywnym oknie Windows.

Start w Studio włącza harmonogram. Pierwszy termin przypada po pełnym zapisanym
interwale (domyślnie 30 min); przygotowanie RSS → scenariusz → mowa rusza od razu
w tle. Gdy nagranie jest gotowe, harmonogram czeka do terminu i wywołuje istniejące
audio_play z resourceId i musicSessionId. Natywny odtwarzacz
sam ścisza i przywraca muzykę; UI nie implementuje fade ani nie zmienia Windows Audio.
„Przygotuj bez odtwarzania” pozwala przejrzeć scenariusz i źródła przed odsłuchem.
Gotowe audio można odtworzyć ponownie bez nowej generacji. Nowy model, głos, tematy
lub docelowy czas audycji unieważniają gotowy podgląd w UI; natywny cache pozostaje.
Operacje przygotowania mogą korzystać z płatnych API. Nie ma automatycznych ponowień.

`ManualBroadcastRunner` realizuje przygotowanie i pojedynczy odsłuch; nowy
`AutomaticBroadcastScheduler` używa go dla kolejnych cykli.
Stop anuluje syntezę albo zatrzymuje nagranie i czeka na zakończenie natywnego play,
które obejmuje przywracanie muzyki. Gdy trwa nieanulowalna generacja tekstu, czeka na
jej odpowiedź i pomija kolejne etapy; UI pokazuje to w statusie. Historia news_mark_used
jest zapisywana dopiero po potwierdzonym zakończeniu odsłuchu. Widok ostatniej audycji
pokazuje wyłącznie faktyczne zakończone odsłuchy z bieżącej sesji UI, bez fikcyjnej
lub domniemanej historii po restarcie. Błąd zapisu historii nie usuwa informacji,
że nagranie zostało odtworzone.

Scheduler ma jawne fazy idle, preparing, waiting, ducking, playing, restoring i error.
Odliczanie i timer należą do klasy poza renderowaniem React. Zegar monotoniczny liczy
interwał, a zegar ścienny i zdarzenia focus/pageshow/visibilitychange wykrywają
przekroczony termin po uśpieniu Windows. Spóźniona audycja lub audycja bez gotowego
audio jest pomijana bez ściszania muzyki. Po faktycznym końcu odsłuchu i przywróceniu
głośności zaczyna się kolejny pełny interwał i przygotowanie następnej audycji.
Zniknięcie źródła zatrzymuje automatyczne przygotowanie do ponownego wyboru sesji.
Stop anuluje harmonogram i czeka na natywne przywrócenie głośności. Logi diagnostyczne
zawierają wyłącznie nazwy zdarzeń, bez treści audycji i sekretów. Ostatnie 100 wpisów
można odczytać przez `getDiagnostics()`; zdarzenia trafiają też do konsoli. Nie ma automatycznego
startu po uruchomieniu aplikacji. Odsłuch testowy i test wyjścia audio pozostają dostępne,
gdy harmonogram jest zatrzymany.

Kontrole UI: `npm run build`, `npm run test:manual`, `npm run test:scheduler` (wbudowany node:test, bez nowych
bibliotek). Testy sprawdzają kolejność etapów, anulowanie, błędy, brak równoległych
uruchomień, wykorzystanie gotowego audio i historię dopiero po realnym końcu play.
Przegląd responsywności w przeglądarce i Tauri obejmuje szerokości 360–1440 px.
W zwykłej przeglądarce nie ma Tauri IPC: puste stany są jawne i nie odtwarzają danych
z SQLite ani stanu Windows. Do testu rzeczywistych serwisów służy `npm run tauri dev`.

### Powiadomienia

Wspólny mechanizm jest w `src/app/notifications`, bez dodatkowych bibliotek.
`notifications.success/error/warning/info(message, { title?, key? })` publikuje
zdarzenia, a jeden `ToastViewport` w korzeniu aplikacji wyświetla je przez portal
nad interfejsem, w prawym dolnym rogu. Niezależne akcje tworzą osobne powiadomienia;
opcjonalny `key` scala powtarzające się błędy z tego samego źródła. Odczyt stanu
odtwarzacza zgłasza ten sam błąd tylko raz do czasu odzyskania połączenia.

Sukces i informacja znikają po 5 sekundach; błąd i ostrzeżenie wymagają zamknięcia X.
Odliczanie jest wstrzymywane podczas najechania, fokusu w stosie i nieaktywności
okna. F8 przenosi fokus do powiadomień; Escape na przycisku zamknięcia zamyka toast.
Animacje uwzględniają `prefers-reduced-motion`. Długie komunikaty i stos można
przewijać. Powiadomienia nie zapisują danych na dysku i nie otrzymują sekretów.

Walidacja pól, niezapisane zmiany, etapy audycji, nieaktywne funkcje i informacje
o ograniczeniach materiałów pozostają przy odpowiednich kontrolkach. Wynik stanu
klucza zachowuje trwałą etykietę diagnostyczną; potwierdzenia operacji trafiają do toastów.
Kontrola magazynu powiadomień: `npm run test:notifications`.

### Odzyskiwanie przygotowanej audycji i częściowe niepowodzenia

Migracja `008_pipeline_recovery.sql` dodaje `pipeline_preparations`,
`playback_receipts` i `speech_segments`. Nie resetuje historii ani ustawień.
Warstwa domenowa używa portu `PreparationRecovery`; adapter Tauri udostępnia:

- `broadcast_restore()` → `{ script, audio: SpeechAsset | null } | null`: najnowsza
  bezpieczna, niewykorzystana audycja lub scenariusz oczekujący na TTS. Sprawdzamy
  do 20 ostatnich zgodnych przygotowań z ostatnich 6 godzin. Zgodność obejmuje tematy,
  model, głos, długość i preferencje materiałów; zmiana sesji muzycznej jej nie unieważnia.
- `broadcast_claim({ broadcastId })`: transakcyjny zapis `attempted` przed play.
  Druga próba tego samego odsłuchu jest blokowana również po restarcie aplikacji.
- `broadcast_complete({ broadcastId })`: zapis `completed` po rzeczywistym końcu
  natywnego play i przywróceniu muzyki. `news_mark_used` pozostaje historią emisji.

Bezpieczny gotowy materiał ma pierwszeństwo przed kolejnym płatnym przygotowaniem.
Przycisk przygotowania nowej audycji nadal próbuje stworzyć nową, zachowując poprzednią
gotową jako fallback na wypadek niepowodzenia. Zapisany scenariusz wznawiany po błędzie
TTS nie jest generowany ponownie. Anulowanie nie uruchamia fallbacku i nie usuwa
poprawnie przygotowanej pracy. Po błędzie historii zakończony odsłuch pozostaje
zabezpieczony przez receipt; nie zgłaszamy błędu całego harmonogramu.

Niepełne audio nie jest odtwarzane. Obecne części TTS mogą dzielić jedną wiadomość,
więc wycięcie losowej części mogłoby zmienić jej sens. Zachowujemy poprawne PCM,
wznawiamy brakujące części przy kolejnej próbie, a jeśli istnieje pełny fallback,
korzystamy z niego. Wszystkie części i cała transkrypcja nadal muszą przejść walidację.
Po awarii procesu nie znamy faktycznej liczby usłyszanych próbek: konserwatywnie nie
powtarzamy także audycji `attempted`, choć mogła nie zacząć grać. Rzeczywisty backend
odtwarzania i odzyskiwanie głośności nie zostały zmienione.

Brak nowych lub bezpiecznych wiadomości jest stanem `waiting`, `ready=false`,
z czytelnym komunikatem. Termin bez nagrania pomijamy bez duckingu; kolejny cykl
działa normalnie. Błąd autoryzacji, niedostępność API, brak możliwości spójnego zapisu
SQLite lub błąd odtwarzania bez bezpiecznej alternatywy nadal są raportowane.
`content_generate` odrzuca Promise obiektem `{ code, message }`, gdzie
`no_usable_material` oznacza zwykły brak materiałów, a `content_failure` awarię usługi.

Diagnostyka `[recovery]` zawiera `stage`, numer segmentu/źródła, `action` i `reason`,
np. `unsupported_evidence`, `repeated_narration`, `reordered`, `reused_verified_pcm`,
`reindexed_verified_wav`. `[perf]` zachowuje pomiary etapów. Logi nie zawierają klucza
ani pełnych odpowiedzi API. Naprawa jednego segmentu odbywa się lokalnie; nie dodajemy
wywołań AI w celu naprawy kolejności lub metadanych.

Testy Rust oraz `test:manual`/`test:scheduler` obejmują wadliwe źródła i segmenty JSON,
powtórki, aktualizacje liczb, kolejność, krótszy scenariusz, wyścig o źródło, brak
materiałów, wznowienie TTS, odzyskiwanie WAV po błędzie SQLite, cache po awarii,
anulowanie, wyłączność odsłuchu i kolejny cykl po niepowodzeniu. Testy używają lokalnych
fixture i atrap dostawców, bez płatnych żądań. Walidacja cytatów i heurystyki podobieństwa
nie zastępują niezależnego sprawdzania faktów; ta gałąź nadal dostarcza materiały RSS,
nie pełne artykuły wydawców.
