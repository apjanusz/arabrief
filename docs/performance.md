# Wydajność przygotowania AraBrief

## Zakres aktualnego kodu

Po cofnięciu wcześniejszych zmian ten checkout zawiera RSS → scenariusz → Realtime
→ WAV. Nie zawiera adaptera pobierania, rozwiązywania i ekstrakcji pełnych artykułów.
Obecność migracji 006–007 utrzymuje zgodność danych, nie oznacza aktywnej integracji.
Ponowne wdrożenie pełnych materiałów wymaga rozstrzygnięcia zakresu; pomiary tej
nieistniejącej warstwy są niedostępne. Nie należy traktować obecnego RSS jako
równoważnika pełnych artykułów.

## Pomiary

Każda operacja natywna używa `Instant`, a cały pipeline i etapy IPC używają
`performance.now()`. ID wpisu identyfikuje pomiar; nie jest ID konta ani klucza.
Logi nie zawierają promptów, transkrypcji, PCM, odpowiedzi API ani nagłówków autoryzacji.

| Operacja | Co obejmuje |
| --- | --- |
| `prepare.total` | Całe przygotowanie bez odtwarzania, widziane przez klienta |
| `prepare.news/content/speech` | Etapy IPC, łącznie z narzutem komunikacji |
| `broadcast.total` | Ręczne przygotowanie i odtworzenie albo sam odsłuch gotowego audio |
| `rss.http` | Żądanie i odczyt ograniczonej odpowiedzi |
| `rss.parse` | Parsowanie i normalizacja kanału |
| `rss.fetch_queue` | Praca maksymalnie czterech pracowników |
| `rss.persist_sqlite/candidates_sqlite` | Zapis wyników oraz wybór nowych wiadomości |
| `rss.total` | RSS wraz z cache/limiterem; `cache_hit` oznacza brak nowych żądań |
| `content.context_sqlite/request_build` | Historia, źródła i przygotowanie JSON |
| `content.responses_http/response_parse` | Rzeczywiste wywołanie modelu i parsowanie wyniku |
| `content.validation/persist_sqlite/total` | Walidacja, transakcyjny zapis i cały serwis |
| `speech.prepare_sqlite/cache_verify` | Przygotowanie, sprawdzenie pliku, SHA i transkrypcji |
| `realtime.connect_credentials_tls/session_setup` | Połączenie i konfiguracja Realtime |
| `realtime.segment_audio_transcript` | Pojedyncza odpowiedź audio wraz z walidacją |
| `speech.persist_wav_sqlite/total` | Zapis WAV i metadanych albo powrót z cache |

Czasy zewnętrznych usług trzeba mierzyć na rzeczywistym uruchomieniu. Czasy
testowych dostawców nie są opóźnieniami OpenAI. Zagnieżdżonych czasów nie należy
dodawać do czasu całkowitego, a równoległe żądania częściowo nakładają się w czasie.

## Porównanie kontrolowane — 16.11.2026

Ten sam zestaw 8 opóźnień `[80, 5, 5, 5, 80, 5, 5, 5]` ms, czterech pracowników,
pięć powtórzeń, mediana. Zwracane dane i ich kolejność są sprawdzane jako identyczne.
Jest to odtwarzalny test oczekiwania na sieć, a nie benchmark Google/OpenAI.

| Pomiar | Przed | Po |
| --- | ---: | ---: |
| Sztywne partie RSS / dynamiczna kolejka | 161,883 ms | 86,150 ms |
| JSON źródeł na dwuelementowej fixture | 704 bajty | 468 bajtów |
| Rzeczywiste generowanie scenariusza i TTS | Nie zmierzono | Dostępne pomiary przy następnym wywołaniu |
| Pełne artykuły | Brak implementacji | Wymaga decyzji o ponownym wdrożeniu |

Potwierdzone wąskie gardło: wolne zadanie zatrzymywało całą kolejną partię, choć
pozostali pracownicy skończyli pracę. Kolejka usuwa tę barierę bez zwiększania
limitu współbieżności, zmiany timeoutów lub pomijania tematów. HTTP 429/503
zatrzymuje pobieranie następnych zadań; rozpoczęte żądania kończą się normalnie.
Limity odpytywania i zapis historii pozostają obsługiwane przez repozytorium.

Pojedynczy rzeczywisty odczyt Google News dla „nauka” zwrócił 100 wpisów:
HTTP 520,401 ms, parsowanie 9,923 ms, cały temat 530,456 ms. To pomiar po dodaniu
diagnostyki; nie jest parą przed/po ani dowodem przyspieszenia Google. W tym odczycie
dominowała sieć. Dla 1–4 tematów wcześniejszy kod już pobierał równolegle, więc
usunięcie barier między partiami dotyczy głównie większej liczby tematów.

Z JSON wysyłanego do modelu usunięto wyłącznie metadane techniczne: dostawcę
adaptera, GUID, adres kanału i czas pobrania. Wszystkie pola zawierające fakty,
daty, tematy, identyfikatory i pochodzenie pozostają identyczne. Liczba bajtów nie
jest liczbą tokenów i nie dowodzi konkretnego skrócenia czasu odpowiedzi OpenAI.

Modele, głos, PCM24k, segmentacja, systemowe instrukcje i walidacje nie są zmienione.
Nie dodano płatnych wywołań ani automatycznych ponowień. Scenariusz zależy od
wiadomości, a TTS od zatwierdzonego tekstu — tych etapów nie da się bezpiecznie
uruchamiać równolegle. Segmenty Realtime pozostają sekwencyjne do czasu pomiaru
i sprawdzenia jakości innej strategii.

## Przygotowanie w interwale i anulowanie

Scheduler już rozpoczyna przygotowanie na początku interwału i przechowuje gotowe
audio do terminu. Testy potwierdzają, że odsłuch gotowego materiału omija RSS,
model tekstowy i TTS. Cache WAV zachowuje walidację SHA oraz transkrypcji.
Ograniczenia jednego zadania i istniejące anulowanie Realtime pozostają aktywne.
Zatrzymanie podczas blokującego ContentGenerator czeka na odpowiedź lub timeout,
a następnie tłumi TTS/odtwarzanie. Nie jest to przerwanie żądania Responses w locie.
Pomiary same nie rozwiązują tego istniejącego ograniczenia.

## Powtórzenie porównania

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib work_queue::tests::compare_batch_barrier_and_work_queue -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib content::tests::compare_material_payload_without_removing_source_facts -- --nocapture
npm run test:manual
npm run test:scheduler
```

Do rzeczywistego profilu uruchom `npm run tauri dev`, wykonaj jedno przygotowanie
i porównaj `prepare.total`, `content.responses_http` oraz czasy segmentów Realtime.
Takie przygotowanie używa istniejących płatnych usług; powyższe testy lokalne nie.
