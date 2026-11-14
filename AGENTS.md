# Zasady pracy nad AraBrief

- Stos: Windows, Tauri 2, React, TypeScript, Vite, Tailwind. Bez Next.js i backendu serwerowego.
- Zachowuj modularność: UI nie wywołuje Windows API. Domena i interfejsy nie zależą od React/Tauri.
- Adaptery dostarczają AudioController, NewsProvider, ContentGenerator, SpeechProvider i SettingsRepository; BroadcastScheduler je orkiestruje.
- Nie implementuj audio, RSS ani API bez osobnego zadania. Nie udawaj aktywnych integracji w UI.
- Muzyka zawsze gra dalej. Nie dodawaj pause/resume. Ściszaj tylko wybrane sesje, nigdy globalne urządzenie.
- Snapshot głośności należy do Rust. Przywracaj po sukcesie, błędzie, anulowaniu i zamknięciu; operacje przywracania mają być idempotentne.
- Jedna audycja naraz. Przygotuj treść i mowę przed ściszeniem; zwalniaj zasoby i subskrypcje.
- Kontrakty IPC utrzymuj w src/platform/tauri/contracts.ts, a po wdrożeniu synchronizuj z Rust DTO. Waliduj dane po obu stronach granicy IPC.
- Małe zależności: preferuj platformę i istniejące biblioteki. Nie dodawaj bibliotek bez potrzeby.
- Bez sekretów w repozytorium, logach, SQLite i frontendowych zmiennych VITE_*. Klucz OpenAI wpisuje się w natywnym oknie Windows i przechowuje wyłącznie w Windows Credential Manager. Nie dodawaj komendy zwracającej klucz do webview. Przyszłe zapytania API wykonuj w Rust.
- Zmiany schematu SQLite dodawaj jako kolejne migracje w src-tauri/migrations; nie zmieniaj już wydanych migracji. Waliduj ustawienia w Rust, zapisuj je transakcyjnie i utrzymuj zgodność DTO TypeScript.
- UI po polsku, dostępne semantycznie i responsywne. Stan ma odpowiadać rzeczywistym możliwościom aplikacji.
- Po zmianach uruchom npm run build oraz odpowiednie cargo fmt/check/clippy. Dla logiki harmonogramu i odzyskiwania audio dodawaj testy błędów i anulowania.
- Aktualizuj README przy zmianie architektury, komend lub uruchamiania. Raportuj kontrole, które nie mogły się wykonać.
- Nie wykonuj commita bez wyraźnego polecenia użytkownika. Nie nadpisuj cudzych zmian.
