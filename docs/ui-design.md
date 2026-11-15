# AraBrief — kierunek wizualny

Punktem odniesienia są oryginalne SVG z przekazanego pakietu logo, zapisane w
`src/assets/brand`. Nie odtwarzamy symbolu papugi ani napisu zastępczą ikoną.
Logo ma prostą geometrię, zaokrąglone słupki audio i mocne, płaskie kolory.
Interfejs rozwija ten charakter jako osobiste studio radiowe: spokojne tło,
wyrazista typografia, jedna główna akcja i dyskretne informacje pomocnicze.

## Tokeny

`src/styles/tokens.css` jest źródłem kolorów marki, kolorów semantycznych,
odstępów, promieni, typografii i czasu przejść. `src/App.css` definiuje komponenty
i układ, a Tailwind korzysta z tych samych tokenów przez `@theme inline`.

| Kolor z logo | Rola w interfejsie |
| --- | --- |
| Granat `#0e1420` | Rama aplikacji i podstawa ciemnego motywu |
| Biel papierowa `#f6f4ef` | Główna typografia |
| Czerwień `#e5322d` | Logo, znacznik aktywnej nawigacji, zatrzymanie |
| Żółty `#ffc531` | Główne akcje, wartości głośności, aktualny etap |
| Niebieski `#1f6fe0` | Elementy marki; jaśniejszy wariant do linków i fokusu |
| Zielony `#1fa971` | Elementy marki; jaśniejszy wariant do potwierdzeń |

Kolory stanu zawsze towarzyszą opisom, numerom lub ikonom. Teksty wymagające
kontrastu korzystają z jaśniejszych wariantów palety. Brak gradientów, animowanych
dekoracji, zewnętrznych fontów i nowych bibliotek. Przejścia przy interakcji trwają
140 ms; `prefers-reduced-motion` je wyłącza.

## Widoki i zachowanie

- **Twoje studio**: dominująca akcja Start/Stop harmonogramu, wybrane źródło,
  rzeczywiste odliczanie, etapy przygotowania, faktyczny ostatni odsłuch,
  tematy i rozwijane źródła. Nie pokazujemy fikcyjnego wizualizera.
- **Zainteresowania**: formularz dodawania i osobna lista zapisanych tematów,
  walidacja przy polu, automatyczny zapis istniejącym repozytorium.
- **Ustawienia**: sekcje z opisem po lewej i kontrolkami po prawej; skróty do
  sekcji, suwaki z czytelnym wynikiem, szczegóły sesji dostępne na żądanie.
  Pasek zapisu pozostaje przy dolnej krawędzi tylko przy niezapisanych zmianach.
  Przełączenie zakładki nie odmontowuje formularza. Zapis przez repozytorium
  synchronizuje formularz z nowymi ustawieniami.
- **Stany**: brak połączenia ma własny stan i ponowienie; błędy operacji oraz
  potwierdzenia trafiają do globalnego stosu toastów. Sukcesy i informacje znikają
  po 5 sekundach, błędy i ostrzeżenia wymagają zamknięcia. Toasty są portalem
  poza układem strony; mają cztery warianty ikon i kolorów, animacje oraz obsługę
  klawiatury. Walidacja i niezapisane zmiany pozostają przy formularzach.
  Puste listy wyjaśniają kolejny krok. Wyłączone działania
  nie udają dostępnych funkcji. Klucz nadal wpisuje się wyłącznie w oknie Windows.

Domyślne okno: 1200 × 820, minimalne: 360 × 480. Układ przechodzi do jednej
kolumny, a przy szerokości do 760 px boczna nawigacja zmienia się w górną.
Interfejs ma podpisane pola, semantyczne grupy ustawień, komunikaty statusu,
widoczny fokus klawiatury i link pomijania nawigacji. Zmiana widoku przenosi
fokus na nagłówek, zachowując normalną kolejność tabulacji.

Backend, adaptery API, kontrakty IPC i mechanika audio pozostają bez zmian.
