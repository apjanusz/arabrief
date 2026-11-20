import { useEffect, useId, useState } from "react";
import { newsCountries } from "../../domain/newsRegion";
import { useCopy, useTranslation } from "../i18n";

export function CountryPicker({ value, onChange }: { value: string; onChange: (code: string) => void }) {
  const copy = useCopy();
  const { language } = useTranslation();
  const id = useId();
  const names = new Intl.DisplayNames([language], { type: "region" });
  const countries = newsCountries.map(country => ({code: country.code, name: names.of(country.code) ?? country.code})).sort((a,b)=>a.name.localeCompare(b.name, language));
  const selected = countries.find(country => country.code === value)?.name ?? "";
  const [query, setQuery] = useState(selected);
  useEffect(() => setQuery(selected), [selected]);
  const match = countries.find(country => country.name.toLocaleLowerCase(language) === query.trim().toLocaleLowerCase(language) || country.code.toLowerCase() === query.trim().toLowerCase());
  return <label className="field"><span>{copy("newsRegionLabel")}</span>
    <input required list={`${id}-countries`} value={query} placeholder={copy("countrySearch")} autoComplete="off" aria-describedby={`${id}-hint`} onFocus={event=>event.currentTarget.select()} onChange={event=>{
      const text = event.target.value;
      setQuery(text);
      const country = countries.find(country => country.name.toLocaleLowerCase(language) === text.trim().toLocaleLowerCase(language) || country.code.toLowerCase() === text.trim().toLowerCase());
      event.currentTarget.setCustomValidity(country ? "" : copy("countryRequired"));
      if (country) onChange(country.code);
    }} ref={input=>{input?.setCustomValidity(match ? "" : copy("countryRequired"));}}/>
    <datalist id={`${id}-countries`}>{countries.map(country=><option key={country.code} value={country.name} label={country.code}/>)}</datalist>
    <small id={`${id}-hint`}>{copy("newsRegionHint")}</small>
  </label>;
}
