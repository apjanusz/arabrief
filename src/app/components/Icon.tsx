import symbol from "../../assets/brand/AraBrief-symbol.svg";

const paths = {
  info: "M12 11v6 M12 7h.01 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z",
  error: "m9 9 6 6 M9 15l6-6 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z",
  warning: "m12 3 10 18H2L12 3Z M12 9v5 M12 17h.01",
  dashboard: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
  topics: "M12 21s-9-5.5-9-12a5 5 0 0 1 9-3 5 5 0 0 1 9 3c0 6.5-9 12-9 12Z",
  settings: "M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1 1-3Z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  play: "m8 5 11 7-11 7V5Z", pause: "M8 5v14 M16 5v14", stop: "M6 6h12v12H6z",
  music: "M9 18V5l12-2v13 M9 9l12-2 M9 18c0 4-7 4-7 1s7-4 7-1Z M21 16c0 4-7 4-7 1s7-4 7-1Z",
  clock: "M12 8v5l3 2 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Z",
  radio: "M4 8h16a2 2 0 0 1 2 2v10H2V10a2 2 0 0 1 2-2Z M4 8l13-6 M7 12a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z M14 12h5 M14 16h5",
  refresh: "M20 7v5h-5 M4 17v-5h5 M5 8a8 8 0 0 1 13-3l2 2 M19 16a8 8 0 0 1-13 3l-2-2",
  plus: "M12 5v14 M5 12h14", close: "m6 6 12 12 M6 18 18 6",
  arrow: "M5 12h14 m-5-5 5 5-5 5", check: "m5 12 4 4L19 6",
  volume: "m3 9 5 0 5-5v16l-5-5H3V9Z M17 8a6 6 0 0 1 0 8 M20 5a10 10 0 0 1 0 14",
  key: "M15 6a5 5 0 1 1-3 9L6 21H2v-4l6-6a5 5 0 0 1 7-5Z M17 8h.01",
  spark: "m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3 3-7Z",
};
export function Icon({ name, size = 20 }: { name: keyof typeof paths; size?: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={paths[name]} /></svg>;
}
export function BrandMark() {
  return <img className="brand-symbol" src={symbol} alt="" aria-hidden="true" width="72" height="72" />;
}
