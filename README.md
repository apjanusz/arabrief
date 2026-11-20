# AraBrief

Aplikacja desktopowa Windows: Tauri 2, React 19, TypeScript 6, Vite 8 i Tailwind CSS 4.

Ustawienia języka interfejsu: Automatycznie (domyślnie), Polski lub English. W trybie automatycznym Rust odczytuje język interfejsu użytkownika Windows (`GetUserDefaultUILanguage`): polski → `pl`, pozostałe → `en`. Ręczny wybór ma pierwszeństwo i jest zapisywany po użyciu „Zapisz ustawienia”. Migracja 014 dodaje `ui_language` z wartością `auto`, zachowując dotychczasowe ustawienia, w tym niezależny język audycji. Centralne słowniki i hook `useTranslation` znajdują się w `src/app/i18n.tsx`, reguły wyboru w `src/domain/language.ts`, a odczyt systemowy w adapterze Tauri. Na tym etapie tłumaczona jest tylko sekcja wyboru języka; pozostałe teksty i audycje pozostają bez zmian. Po zmianie języka Windows należy ponownie uruchomić AraBrief.
Po uruchomieniu harmonogramu okresowo ścisza muzykę innej aplikacji, odtwarza krótką audycję AI i przywraca
wcześniejszą głośność. Muzyka nigdy nie jest pauzowana.

## Instalacja MVP 0.1.0 — Windows

Wydanie przeznaczone jest dla Windows 10/11 x64. Uruchom `AraBrief_0.1.0_x64-setup.exe` i przejdź przez instalator NSIS. Standardowe okna są po polsku; część dodatkowych komunikatów Tauri pozostaje po angielsku. Instalacja dotyczy bieżącego użytkownika. Jeśli brakuje Microsoft Edge WebView2 Runtime, instalator pobierze jego instalator — ten krok wymaga internetu i może wymagać dodatkowych uprawnień. Do działania zainstalowanej aplikacji nie potrzeba Node.js, Rust ani katalogu projektu. Wydanie MVP nie jest podpisane certyfikatem; Windows może wyświetlić ostrzeżenie SmartScreen.

Po uruchomieniu otwórz Ustawienia, zapisz własny klucz OpenAI API w natywnym oknie Windows i sprawdź połączenie. Klucz trafia wyłącznie do Windows Credential Manager, nie do SQLite ani instalatora. Korzystanie z API wymaga aktywnego rozliczania OpenAI i może generować opłaty. Wybierz aktywną sesję muzyki, dodaj tematy i uruchom harmonogram. **Głos audycji jest generowany przez sztuczną inteligencję (OpenAI TTS), nie przez człowieka.**

Dane znajdują się w katalogu użytkownika `%APPDATA%\com.janus.ara-brief`: baza `settings.sqlite3` przechowuje ustawienia, kolejkę, historię i cache rozdziałów/fragmentów PCM, a podkatalog `audio` gotowe nagrania. Profil WebView2 jest zarządzany przez Tauri w katalogach użytkownika. Zachowano identyfikator `com.janus.ara-brief`, aby utrzymać dostęp do dotychczasowych danych i poświadczeń. Instalator nie zawiera danych autora ani klucza API; istniejące dane developerskie tego samego użytkownika mogą zostać wykorzystane przez zainstalowany program.

Build: `npm ci`, następnie w środowisku z Rust MSVC i Visual Studio Build Tools `npm run tauri -- build --bundles nsis`. Wynik domyślnie trafia do `src-tauri/target/release/bundle/nsis/`. Frontend, logo, ikony, prompty i migracje są osadzane podczas kompilacji; SQLite jest kompilowane przez funkcję `bundled`. Nie dodajemy katalogu projektu do `bundle.resources`.

Krótka checklista instalatora (do wykonania ręcznie na Windows, najlepiej na świeżym koncie):

- [ ] Instalacja bez narzędzi developerskich, uruchomienie ze skrótu i poprawne logo; sprawdzenie wariantu bez WebView2.
- [ ] Zapis klucza, ustawień i tematów; ponowne uruchomienie zachowuje dane, kolejkę i gotowe audio.
- [ ] Przygotowanie oraz odsłuch audycji; poprawny głos i przywrócenie głośności muzyki po końcu oraz błędzie.
- [ ] Pauza/Wznów podczas audycji i odliczania; Stop, ukrycie do zasobnika i rzeczywiste zakończenie aplikacji.
- [ ] Uśpienie/wybudzenie; autostart wyłączony domyślnie, brak samoczynnego odtwarzania bez konfiguracji.
- [ ] Ponowna instalacja i odinstalowanie po zamknięciu aplikacji; sprawdzenie skrótów, autostartu i sposobu zachowania/usuwania danych użytkownika.

## Obecny zakres

Limity kosztów rozdziałów: Responses API zachowuje `max_tool_calls: 1`, ale otrzymuje rzeczywisty tekst artykułu pobrany natywnie i `tool_choice: none`. Nowy przepływ wykonuje zatem **zero płatnych web_search** i jedno zapytanie tekstowe na rozdział, bez automatycznego ponawiania HTTP OpenAI. Model dostaje treść, URL i metadane, zamiast próbować znaleźć i otworzyć link w jednym wywołaniu narzędzia. Nadal naliczane są zwykłe koszty tokenów modelu i TTS. Zachowano historyczny rejestr wykorzystanych prób web_search z migracji 012; nie jest resetowany ani obchodzony nowym wyszukiwaniem.

Natywny czytnik (`content/article.rs`) rozwiązuje Google News do strony wydawcy, sprawdza zgodność domeny i tytułu, pobiera HTML i wybiera `articleBody` z JSON-LD lub akapity treści. Google używa niedokumentowanego protokołu przekierowania `Fbv4je`/`garturlreq`; jego zmiana może wymagać aktualizacji resolvera. Nie ma dodatkowej usługi, Node.js w działającej aplikacji ani klucza wyszukiwarki. Parser HTML to `dom_query`, już używany przechodnio przez Tauri, teraz jawna zależność czytnika. Tekst jest zapisywany w istniejącej tabeli `article_materials` przed AI, więc błąd modelu nie usuwa pobranego materiału. Strony z jawnym oznaczeniem paywalla, krótkie zajawki, blokady HTTP i strony wymagające renderowania JavaScript są pomijane. Czytnik nie omija blokad wydawcy. Model nie może udawać osobistego otwarcia strony: `article_read` oznacza przeczytanie dostarczonego tekstu, a cytaty są sprawdzane lokalnie w tym tekście.

Ograniczenia pobierania: publiczne adresy HTTP(S), zweryfikowana domena, kontrola adresów DNS i każdego przekierowania, maksymalnie 2 MiB strony i 24 tys. znaków treści, UTF-8, timeout HTTP do 12 s i budżet czytnika 40 s (systemowe rozwiązywanie DNS ma własne ograniczenia). Połączenia są bezpośrednie, bez proxy i bez ciasteczek przeglądarki użytkownika. Migracja 013 jednorazowo przywraca do kolejki niewykorzystane artykuły odrzucone przez poprzednią ścieżkę web_search, o ile nie mają udanego rozdziału ani historii rozpoczęcia/odsłuchu. Nie usuwa historii i rejestru kosztów. Test na żywo bez OpenAI: `cargo test --manifest-path src-tauri/Cargo.toml --lib live_google_article_reader_without_openai -- --ignored --nocapture` (wymaga internetu).

Tekst pojedynczego rozdziału ma najwyżej 100 słów oddzielonych białymi znakami. Limit jest zapisany w prompcie oraz egzekwowany lokalnie, również dla cache rozdziałów: nadmiar jest usuwany do końca ostatniego pełnego zdania w limicie. Brak pełnego zdania oznacza nieudany rozdział, bez dodatkowego zapytania AI. Nie zmieniono TTS ani harmonogramu; ustawiony czas audycji nie wymusza przekroczenia limitu słów. Stare gotowe audycje nie są automatycznie odzyskiwane pod nowym kluczem konfiguracji.

Dobór tematów: zapytania Google News używają jawnych wyrażeń, a przed zapisaniem Google RSS do kolejki lokalny filtr sprawdza nagłówek i opis (bez nazw wydawców i atrybutów HTML). Dla wspinaczki uwzględnia również bouldering, a dla diety sportowej wymaga kontekstu żywieniowego i sportowego. Dla pozostałych tematów wymaga istotnych słów z uwzględnieniem prostych odmian. Nie wykonuje dodatkowych zapytań AI ani semantycznej deduplikacji. To filtr słownikowy, nie gwarancja rozumienia pełnej treści: może pominąć trafny artykuł opisany wyłącznie synonimem. Brak trafnych wiadomości jest lepszy niż zastępowanie ich ogólnymi wiadomościami sportowymi. Przy pobieraniu usuwa nietrafione przypisania ze starej kolejki, zachowując historię i limity żądań. Zmieniono klucz odzyskiwania przygotowania, aby nie wracały gotowe audycje przygotowane przed wprowadzeniem filtra; zapisane fragmenty rozdziałów i audio nadal mogą być ponownie wykorzystane dla poprawnych artykułów.

Polski interfejs z harmonogramem, wyborem sesji, zainteresowaniami i ustawieniami.
Synteza mowy przez OpenAI Speech API, Google News RSS i generowanie scenariuszy
przez OpenAI Responses API działają natywnie. Ustawienia są zapisywane w SQLite,
a klucz API w Windows Credential Manager.
Odczyt sesji, ducking i natywne odtwarzanie WAV/MP3 są aktywne. Dostępny jest test
całego cyklu z muzyką i dwutonowym nagraniem WAV.

## Zasobnik, autostart i odzyskiwanie audio

Zamknięcie okna (X / Alt+F4) ukrywa AraBrief. Harmonogram zachowuje termin,
przygotowanie i odtwarzanie działają dalej. Ikona zasobnika ma menu „Otwórz AraBrief”,
„Start” / „Stop” oraz „Zakończ aplikację”. Kliknięcie ikony przywraca okno.
Start jest dostępny po wczytaniu ustawień, wybraniu dostępnej sesji i tematów.
Stop zatrzymuje harmonogram lub bieżącą audycję; muzyka nadal gra.
Ponowne uruchomienie programu otwiera istniejącą instancję.

W ustawieniach dodano dwie niezależne opcje, obie domyślnie wyłączone:

- `autoStart`: oficjalny plugin autostart Tauri rejestruje uruchomienie po zalogowaniu
  do Windows z argumentem `--autostart`; okno pozostaje ukryte. Odczyt ustawień
  sprawdza rzeczywisty stan rejestracji Windows. Zapis waliduje ustawienia,
  aktualizuje autostart i transakcyjnie zapisuje SQLite, z próbą cofnięcia autostartu
  w razie błędu bazy. Rejestracja Windows i SQLite nie tworzą wspólnej transakcji.
- `startSchedulerOnLaunch`: osobna zgoda na start harmonogramu i płatne API po
  uruchomieniu aplikacji. Pierwsza audycja czeka pełny interwał. Bez tej zgody
  uruchomienie z Windowsem ani odzyskiwanie stanu nie rozpoczyna odtwarzania.

Rust utrzymuje zasobnik i wysyła aktywnemu harmonogramowi zdarzenie co sekundę.
Webview pozostaje załadowany po ukryciu. Konfiguracja Tauri `additionalBrowserArgs`
wyłącza ograniczanie timerów i pracy renderera w tle w WebView2; nie korzysta z
`backgroundThrottling`, które nie jest obsługiwane na Windows. Domena harmonogramu
otrzymuje wybudzenie przez port `BackgroundLifecycle`, bez zależności od Tauri.
Uśpienie komputera nadal wstrzymuje pracę; przeterminowana audycja jest pomijana.

„Zakończ aplikację” wstrzymuje `ExitRequested`, blokuje nowe odtwarzanie i nowe
komendy przygotowania, anuluje oba schedulery oraz syntezę, zatrzymuje lektora
i przywraca muzykę w Rust. Natywne sprzątanie działa także przy braku odpowiedzi UI;
oczekiwanie na potwierdzenie warstwy UI ma limit 3 sekund po sprzątaniu audio.

Migracja 009 dodaje `audio_recovery`. Rust zapisuje oryginalną głośność, dokładną
tożsamość endpointu i instancji sesji oraz wartości przed i po planowanym zapisie,
z SQLite `synchronous=FULL`, przed każdym zapisem Core Audio. Dzięki temu awaria
w trakcie fade również pozostawia snapshot. Sukces lub ręczna zmiana głośności
usuwa zapis, błąd API zachowuje go do ponownej próby. Po restarcie odzyskiwanie
wykonuje się przed dopuszczeniem kolejnego ściszenia; nie wznawia starej audycji.

Windows nie daje atomowego odczytu i zapisu głośności względem miksera ani kodu
wykonywanego po wymuszonym zabiciu procesu. Odzyskiwanie następuje przy kolejnym
uruchomieniu i dotyczy wyłącznie tej samej instancji sesji, jeśli jej obecny poziom
odpowiada jednej z zapisanych wartości. Inny poziom pozostaje bez zmian. Nie można
odtworzyć historii zmian miksera podczas przerwy w działaniu AraBrief.
Niedostępny endpoint lub błąd API zachowuje zapis i blokuje nowe ściszenie do
odzyskania; jeśli endpoint jest dostępny, lecz dawna sesja wygasła, zapis jest
usuwany bez zmieniania nowej sesji. Nie ma zmiany globalnej głośności ani pauzowania.

Kontrole: `npm run test:background`, `npm run test:scheduler`, `npm run test:manual`,
`cargo test --manifest-path src-tauri/Cargo.toml --lib` oraz standardowy build i clippy.
Testy obejmują IPC zasobnika, potwierdzenie zamknięcia, anulowanie i błędy odtwarzania,
awarię po obu stronach zapisu audio, ponowne otwarcie SQLite, ręczną zmianę miksera
i niepowodzenie zapisu dziennika. Testy jednostkowe nie zmieniają prawdziwej głośności.
Ręcznie sprawdź działanie z ukrytym oknem przez kilka interwałów, Start/Stop w zasobniku,
wyjście podczas przygotowania/fade/odtwarzania i ponowne uruchomienie po zabiciu
procesu podczas audycji. Autostart wymaga sprawdzenia po ponownym zalogowaniu Windows.

Oficjalne mechanizmy: [zasobnik Tauri 2](https://v2.tauri.app/learn/system-tray/),
[autostart](https://v2.tauri.app/plugin/autostart/),
[single-instance](https://v2.tauri.app/plugin/single-instance/),
[konfiguracja WebView2](https://v2.tauri.app/reference/config/#additionalbrowserargs).

## Uruchomienie na Windows

Wymagania: Node.js 22.12+ (lub obsługiwana nowsza wersja LTS), npm, Rust stable
z targetem MSVC, Visual Studio Build Tools z C++ i Windows SDK oraz WebView2 Runtime.
Narzędzia muszą być dostępne w PATH. Oficjalne wtyczki Tauri wymagają Rust 1.90+.

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
na stderr. Wymuszone zabicie procesu/awaria systemu nie wykonuje kodu zamykania;
trwały snapshot pozwala podjąć próbę odzyskania przy następnym uruchomieniu.

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

Obsługiwany schemat SQLite ma wersję 11. Migracja 009 dodaje dziennik odzyskiwania
audio i opcję startu harmonogramu. Zachowane migracje 006–007 odpowiadają
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
| autoStart | false | bool; oficjalny plugin autostart uruchamia aplikację w zasobniku po zalogowaniu do Windows |
| startSchedulerOnLaunch | false | bool; osobna zgoda na automatyczny harmonogram i płatne przygotowanie audycji |
| language / newsSourceUrls | pl / [] | preferencje przyszłego modułu wiadomości |

`settings_load()` zwraca kompletny AppSettings; `settings_save({ settings })` zapisuje
pełny, zwalidowany snapshot. Numer wersji DTO (version: 1) jest niezależny od wersji
schematu bazy (obecnie 11). Nieznane pola są odrzucane, więc apiKey nie może zostać
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

## Kolejka artykułów i rozdziały audycji (MVP)

Google News jest dostawcą metadanych do trwałej kolejki w SQLite. Pula mieści
standardowo 20 pozycji, a audycja wybiera trzy najstarsze dostępne artykuły
opublikowane w ostatnich siedmiu dniach (7 × 24 godziny).
Kolejne przygotowania korzystają z tej samej kolejki. Przy co najmniej trzech
pozycjach nie wysyłamy nowych zapytań RSS. Gdy pozostają jedna lub dwie pozycje,
uzupełniamy pulę do 20, zachowując pozostałe. Remisy dat rozstrzyga stabilny
lokalny identyfikator. Dla Google News
brak lub błędna data wyklucza przyjęcie artykułu i wybór do nowej audycji.

Zachowujemy wybrane tematy użytkownika. `news_topic_articles` wiąże artykuły
z tematami, a generowanie ponownie sprawdza przynależność do obecnych ustawień.
Zmiana tematów lub głosu nie usuwa historii. Rozdział tekstowy można ponownie
wykorzystać po zmianie głosu; gotowe nagranie nadal musi pasować do głosu.

Migracja `010_article_queue.sql` dodaje:

| Tabela | Zadanie |
| --- | --- |
| `article_queue` | Chronologia, liczba błędów, termin ponowienia i pominięcie wadliwych pozycji |
| `news_pool_state` | Najnowsza data publikacji przyjęta dla każdego tematu |
| `article_chapters` | Poprawny tekst rozdziału, konfiguracja modelu i ślad otwarcia strony |
| `broadcast_chapters` | Kolejność artykułów, teksty i granice czasowe kompletnych rozdziałów audio |

Przy uzupełnianiu puli wysyłamy zapytanie Google News z datą `after:` cofniętą
o 24 godziny względem zapisanego maksimum, z granicą nie starszą niż siedem dni.
Dotyczy to również pierwszego pobrania i starego watermarku po przerwie w działaniu.
Google filtruje po dniu, dlatego dodatkowo sprawdzamy dokładny czas publikacji
w Rust przed zapisem. Starsze zapisane pozycje Google News są pomijane przy wyborze
z kolejki; historia pozostaje w bazie. Identyfikatory, GUID-y i
znormalizowane URL-e zapobiegają powtórkom. Usuwamy fragment URL, typowe parametry
śledzące i porządkujemy parametry. Tytuł, wydawca, podobieństwo semantyczne i
wspólne cytaty nie są kryteriami odrzucenia. Wcześniejsza kolumna `fingerprint`
pozostaje dla zgodności schematu, lecz jej wartości są czyszczone i nieużywane.

Google News RSS nie jest pełnym, stronicowanym archiwum. Czytamy najwyżej 100
pozycji na temat z odpowiedzi do 2 MiB. Opóźnione publikacje spoza nakładającego
się zakresu lub nieobecne w kanale mogą pozostać niewykryte. Pobrania respektują
`Retry-After`; poza cooldownem jest krótka blokada równoległych pobrań, a nie
odpytywanie co pięć minut. Błąd jednego kanału nie usuwa wyników pozostałych.

## Odczytywanie artykułu i generowanie rozdziału

Aktualny przepływ to **jedno niezależne żądanie Responses na artykuł**, z maksymalnie
trzema pracownikami. Nie ma dodatkowych zapytań AI do selekcji, deduplikacji,
łączenia tekstów ani końcowego podsumowania. Dotychczasowy prompt i walidator
scenariuszy z RSS pozostają wyłącznie dla testów zgodności i odczytu dawnych danych;
nowe rozdziały ich nie używają.

`content/chapters.rs` wysyła metadane, końcowy URL wydawcy i rzeczywisty tekst
pobrany przez `content/article.rs`. `tool_choice=none` wyłącza wywołania narzędzi.
Aplikacja wymaga jednocześnie:

- ukończonej odpowiedzi i poprawnego JSON-u;
- `article_read=true`, gotowego tekstu i krótkich fragmentów dowodowych;
- zapisanego natywnego odczytu i dosłownej zgodności cytatów z dostarczoną treścią;
- zgodności URL-a rozdziału z pobraną stroną, domeny wydawcy i tytułu z metadanymi.

Samo `search`, snippet, adres Google News, strona główna wydawcy lub deklaracja
odczytu bez pobranej treści nie wystarcza. Model musi zgłosić niedostępność pełnej
treści, np. przy paywallu. Nie ma zastępczego tworzenia tekstu z samego nagłówka.
Strony traktowane są jako niezaufane dane, nie instrukcje. Źródła w interfejsie
są klikalnymi linkami do przeczytanych artykułów.

Ślad `native_http_article` oznacza pobranie i odczyt strony przez aplikację,
a nie przez narzędzie OpenAI. Kontrola cytatów nie jest niezależną weryfikacją
wszystkich faktów i parafraz. Nie zmieniamy samoczynnie modelu w ustawieniach.

Każdy poprawny rozdział zapisujemy od razu, zanim skończą się pozostałe żądania.
Błąd późniejszego zapisu audycji lub TTS nie usuwa cache tekstu. Po błędzie
artykułu próbujemy kolejnego, najwyżej dziewięciu kandydatów w jednym przygotowaniu.
Jeśli powstaną jeden lub dwa rozdziały, publikujemy krótszą audycję. Trzy nieudane
przygotowania konkretnego artykułu wyłączają go z aktywnej kolejki; pomiędzy próbami
jest 60 sekund przerwy. Globalne błędy klucza/modelu/API nie oznaczają artykułów
jako trwale wadliwych. Żądanie rozdziału ma timeout 90 s; ponawiamy najwyżej raz
HTTP 429/5xx, tylko gdy oczekiwanie `Retry-After` nie przekracza dwóch sekund.
Nie powtarzamy automatycznie niejednoznacznego timeoutu płatnego żądania.

Po tekście następuje bezpośrednio istniejący TTS Realtime, z wybranym głosem.
Każdy rozdział jest dzielony na fragmenty zgodnie z dotychczasowym limitem tekstu
800 znaków. Poprawne PCM pozostają w istniejącym `speech_segments`. Błąd jednego
rozdziału nie przerywa pozostałych. Publikowane są wyłącznie kompletne rozdziały,
w kolejności artykułów; `SpeechAsset.chapters` zawiera `articleId`, `position`,
`startMs`, `endMs`. Wszystkie opublikowane fragmenty muszą przejść walidację
transkrypcji. Nie sklejamy niekompletnych zdań z uszkodzonego rozdziału.

Błąd TTS zachowuje tekst i dostępne PCM. Brak choćby jednego kompletnego rozdziału
zwalnia rezerwację nieudanej audycji, aby nie blokować kolejki. Teksty i fragmenty
pozostają do wznowienia. Mowa rozdziału ma limit 300 sekund na próbę i najwyżej
jedno ponowienie przy błędzie połączenia/timeout; cache zapobiega powtarzaniu
poprawnie zapisanych fragmentów. Historia `news_usage` obejmuje wyłącznie artykuły
z odtworzonego nagrania. Receipt `attempted` nadal zabezpiecza przed powtórką
częściowo usłyszanej audycji po awarii.

Harmonogram jest zachowany: przygotowanie zaczyna się na starcie, gotowe audio
czeka na pełny interwał, a następna audycja zaczyna się przygotowywać po zakończeniu
natywnego odtwarzania i przywróceniu muzyki. Jedna bramka przygotowania scenariusza,
jedna synteza i istniejąca blokada odtwarzacza zapobiegają dublowaniu pracy.
Ducking i przywracanie głośności nie zostały zmienione.

Testy deterministyczne obejmują pulę 20, dwie kolejne audycje bez RSS, uzupełnienie
z zachowaniem dwóch pozycji, wyczerpanie, restart, tolerancyjne duplikaty, niezależne
błędy modelu, częściowe TTS, błędną transkrypcję, cache po błędzie SQLite, chronologię,
granice audio, anulowanie i przygotowanie następnego cyklu. Polecenia:
`cargo test --manifest-path src-tauri/Cargo.toml --lib`, `npm run test:manual`,
`npm run test:scheduler`, `npm run build` oraz standardowe fmt/check/clippy.
Testy używają atrap dostawców; nie wykonują płatnych wywołań OpenAI.

Dokumentacja API: [web search i akcje otwarcia strony](https://developers.openai.com/api/docs/guides/tools-web-search),
[Responses API](https://developers.openai.com/api/reference/resources/responses/methods/create).

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
a nie z czasu oczekiwania. Limit całego audio to 15 minut, a podział pojedynczego rozdziału ma limit 60 fragmentów; limit
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

Niekompletne rozdziały nie są odtwarzane; kompletne rozdziały są publikowane
niezależnie. Aktualny mechanizm opisuje sekcja MVP powyżej. Testy dawnych scenariuszy
RSS nadal sprawdzają zgodność danych historycznych; heurystyki podobieństwa nie
uczestniczą w generowaniu nowych audycji. Diagnostyka `[pipeline]` podaje etap,
identyfikator artykułu i wynik. Logi nie zawierają sekretów ani pełnych odpowiedzi API.

Aktualizacja TTS: adapter korzysta z /v1/audio/speech, modelu gpt-4o-mini-tts i głosów marin/cedar. Migracja 011 zastępuje inne zapisane głosy przez marin. Każde żądanie zawiera instrukcję polskiej wymowy. Podział tekstu, cache PCM, składanie i anulowanie pozostają zachowane. Metadana tts_input_text oznacza tekst wejściowy syntezy, a nie transkrypcję otrzymanego audio (Speech API jej nie zwraca). Starsze sekcje Realtime opisują poprzedni adapter. Model według dokumentacji OpenAI będzie wyłączony 6 stycznia 2027: https://developers.openai.com/api/docs/guides/text-to-speech .

Przegląd stabilności MVP: błędy sieci i timeouty odraczają artykuły bez zwiększania licznika wadliwych źródeł; błąd enumeracji sesji nie blokuje harmonogramu. Anulowanie TTS czeka na zakończenie bieżącego blokującego żądania (timeout HTTP 90 s) i zapis cache. Dodatkowa bramka chroni przed równoległym ponowieniem tego samego fragmentu po timeoutcie zadania. Gdy zapis dziennika SQLite zawiedzie podczas przywracania muzyki, Rust podejmuje bezpośrednią próbę odtworzenia oryginalnego poziomu, respektując zmianę użytkownika.

Pauza/Wznów: przycisk na pulpicie wstrzymuje harmonogram lub trwającą audycję (także odsłuch testowy gotowego nagrania). Pomiędzy audycjami zachowuje pozostały czas i gotowe materiały; rozpoczęte przygotowanie może dokończyć się w tle. Podczas audycji Rust pauzuje tę samą kolejkę rodio i przywraca wybraną sesję muzyki. Wznowienie ponownie ścisza muzykę, pobierając nowy snapshot jej aktualnej głośności, i kontynuuje ten sam strumień bez generowania ani ponownego dekodowania. Muzyka nie jest pauzowana. Polecenie IPC `audio_set_paused { paused, playbackKey }` jest przypisane do konkretnego odtwarzania i potwierdzane po przywróceniu/ściszeniu muzyki; spóźnione polecenie nie steruje następną audycją. Stop i rzeczywiste zakończenie aplikacji zwalniają również wstrzymane odtwarzanie. Pauza obowiązuje w bieżącym procesie, nie jest zapisywana jako ustawienie autostartu.

Test ręczny Windows przed wydaniem: w trakcie audycji kilka razy użyć Pauza/Wznów i sprawdzić ciągłość wypowiedzi oraz mikser muzyki; podczas pauzy zmienić głośność muzyki, wznowić i sprawdzić jej przywrócenie po końcu. Sprawdzić też pauzę odliczania przez uśpienie/wybudzenie, Stop/Zakończ w pauzie oraz odłączenie wyjścia audio przed wznowieniem. Testy automatyczne używają zegara testowego i kolejki rodio bez fizycznego wyjścia — nie zastępują odsłuchu i weryfikacji Windows Audio API.

Logi konsoli: rutynowe komunikaty `[perf]`, `[pipeline]` i `[recovery]` są domyślnie wyłączone także w trybie developerskim. Natywną diagnostykę można jednorazowo włączyć, ustawiając w PowerShell `$env:ARABRIEF_DIAGNOSTICS='1'` przed uruchomieniem procesu (odczyt przy pierwszym logowaniu; zmiana wymaga restartu). Frontend nie wypisuje rutynowych etapów i pomiarów do konsoli; obserwatory pomiarów oraz ograniczona historia diagnostyczna harmonogramu pozostają dostępne. Błędy przywracania audio, zamykania i obsługi aplikacji nadal są raportowane. SQLite zachowuje stan kolejki i cache niezależnie od logowania; nie przechowuje pełnej historii diagnostycznej konsoli.
