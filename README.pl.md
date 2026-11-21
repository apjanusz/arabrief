<p align="center">
  <img src="src/assets/brand/AraBrief-symbol.svg" alt="Logo AraBrief" width="96">
</p>

<h1 align="center">AraBrief</h1>

<p align="center"><strong>Twoje zainteresowania. Twoja muzyka. Twoja osobista audycja.</strong></p>

<p align="center">
  <a href="https://github.com/apjanusz/ara-brief/releases/latest">Pobierz na Windows</a> ·
  <a href="docs/getting-started.md">Pierwsze kroki</a> ·
  <a href="README.md">English</a>
</p>

AraBrief zamienia wiadomości o interesujących Cię tematach w krótkie audycje z głosem AI na pulpicie Windows. Słuchaj dalej muzyki: kiedy audycja jest gotowa, AraBrief ścisza wybraną aplikację muzyczną, odtwarza wiadomości i przywraca wcześniejszą głośność.

## Najważniejsze funkcje

- **Wiadomości zgodne z zainteresowaniami** — wyszukiwanie tematów w Google News RSS i odczytywanie artykułów.
- **Audycje według harmonogramu** — przygotowanie w tle i odtwarzanie w ustawionych odstępach.
- **Muzyka gra dalej** — ściszanie wybranej sesji audio i niezależna głośność lektora.
- **Podgląd przed odsłuchem** — przygotowanie bez odtwarzania, przegląd scenariusza i linków do źródeł.
- **Pauza i wznowienie** — wstrzymanie audycji lub odliczania bez zatrzymywania muzyki.
- **Polski i angielski** — interfejs i narracja w wybranym języku oraz niezależny wybór kraju wiadomości.
- **Wygoda aplikacji desktopowej** — zasobnik systemowy, opcjonalny autostart z Windowsem i lokalny cache tekstu oraz audio.
- **Własny klucz API** — wpisywany w natywnym oknie Windows i przechowywany w Windows Credential Manager.

## Jak to działa

1. Dodaj klucz OpenAI API, wybierz tematy i kraj wiadomości oraz sesję audio aplikacji muzycznej.
2. Uruchom harmonogram. AraBrief wyszuka artykuły, odczyta dostępne treści i przygotuje scenariusz oraz narrację.
3. W wyznaczonym terminie AraBrief ściszy wybraną sesję i odtworzy gotową audycję.
4. Po odsłuchu przywróci głośność muzyki i rozpocznie przygotowanie kolejnego cyklu.

Pierwsza zaplanowana audycja czeka pełny interwał. Możesz też przygotować i odsłuchać ją ręcznie. Głos generuje AI, a nie człowiek; dostępność źródeł i ilość użytecznego materiału wpływają na każdą audycję.

## Zrzuty ekranu

W tym miejscu pojawią się zrzuty aplikacji. Planowane widoki: **Studio**, **Zainteresowania** i **Ustawienia**.

<!-- Dodaj tutaj prawdziwe zrzuty aplikacji po zapisaniu plików graficznych w repozytorium. -->

## Pobieranie

Pobierz instalator Windows x64 z [najnowszego wydania GitHub Releases](https://github.com/apjanusz/ara-brief/releases/latest). W sekcji **Assets** wybierz plik z końcówką `x64-setup.exe`.

Instrukcję konfiguracji znajdziesz w [instalacji i pierwszym uruchomieniu](docs/getting-started.md). Obecne instalatory MVP nie są podpisane certyfikatem, więc Windows może wyświetlić ostrzeżenie SmartScreen.

## Wymagania i koszty API

- Windows 10 lub 11, x64.
- Microsoft Edge WebView2 Runtime; przy jego braku instalator pobierze program instalacyjny tego środowiska.
- Internet do wyszukiwania wiadomości, pobierania artykułów i korzystania z OpenAI.
- Własny klucz OpenAI API, aktywne rozliczanie API lub dostępne środki oraz dostęp do skonfigurowanych modeli tekstu i mowy.

**Generowanie tekstu i synteza mowy wiążą się z opłatami za OpenAI API**, również przy przygotowaniu bez odsłuchu. Rozliczanie API jest niezależne od subskrypcji ChatGPT. Koszt zależy od modeli, danych wejściowych i wyjściowych, wygenerowanej mowy oraz częstotliwości audycji. Sprawdź [aktualny cennik API](https://developers.openai.com/api/docs/pricing) i [wyjaśnienie rozliczeń OpenAI](https://help.openai.com/en/articles/9039756-managing-billing-for-chatgpt-and-the-api-platform).

Obecny adapter mowy korzysta z `gpt-4o-mini-tts`. OpenAI zapowiedziało wyłączenie tego modelu **6 stycznia 2027 r.**; przed korzystaniem z tej wersji po tym terminie przeczytaj [informację o zgodności syntezy mowy](docs/troubleshooting.md#speech-model-availability).

## Dokumentacja

Dokumenty poniżej są po angielsku, z wyjątkiem istniejących notatek o projekcie interfejsu.

| Dla kogo / temat | Przewodnik |
| --- | --- |
| Instalacja i konfiguracja AraBrief | [Pierwsze kroki](docs/getting-started.md) |
| Typowe problemy | [Rozwiązywanie problemów](docs/troubleshooting.md) |
| Dane lokalne i usługi zewnętrzne | [Prywatność](docs/privacy.md) |
| Praca nad projektem | [Development](docs/development.md) |
| Moduły i odpowiedzialności | [Architektura](docs/architecture.md) |
| Wiadomości, generowanie i cache | [Przygotowanie audycji](docs/broadcast-pipeline.md) |
| Windows Audio i IPC | [Integracja natywna](docs/native-integration.md) |
| Pakowanie i publikowanie | [Wydania](docs/releases.md) |
| Pomiary i porównania historyczne | [Wydajność](docs/performance.md) |
| Notatki wizualne po polsku | [Projekt interfejsu](docs/ui-design.md) |

Stos projektu: **Tauri 2, React, TypeScript, Vite, Tailwind CSS i Rust**. Aplikacja działa na komputerze użytkownika bez własnego backendu serwerowego projektu.
