# Licensing and asset review

[Project overview](../README.md) · [Publication review](publication-review.md)

Reviewed November 21, 2026 against the checked-in npm/Cargo lockfiles, locally available package manifests and selected license texts, and the primary sources linked below. This is a publication review, not a complete installer license bundle or a determination of rights in every asset.

## AraBrief license: MIT

The owner selected the [MIT License](../LICENSE). The npm and Cargo manifests declare `MIT`; the copyright notice uses the repository owner's public handle, `apjanusz`. Retain the copyright and permission notice when distributing copies or substantial portions of the software.

This project license does not replace dependency licenses, grant rights in third-party news content, or establish ownership of externally supplied assets. Asset provenance findings below remain relevant. No separate trademark permission is created by this notice.

## Dependency scope

- npm: all **100 package entries** in `package-lock.json`, excluding the application root, have declared license metadata. **54** have installed manifests available for local cross-checking; **46** are absent platform/optional packages reviewed through lockfile metadata only.
- Rust: all **537 external package entries** in `Cargo.lock` have locally cached manifests and license declarations. The lockfile includes other operating systems, build tools, optional packages, and duplicate versions. This is not a list of exactly what was linked into the Windows installer.
- No GPL-only or AGPL-only declaration was found in this metadata inventory. `r-efi` offers LGPL as one alternative alongside MIT/Apache; that alternative is not mandatory. This is not a claim that metadata captures every vendored-file obligation.
- Dependency versions and resolutions are unchanged from tag `v0.1.0`; the npm lockfile's application entry now also declares MIT. Neither the installer payload nor every bundled third-party source file was audited. No dependency was downloaded, changed, or rebuilt for this review.

### Direct dependencies

These versions are the direct application's resolutions, rather than every same-named transitive version.

| Component | Locked version(s) | Declared license |
| --- | --- | --- |
| React, React DOM | 19.3.1 | MIT |
| Tailwind CSS, Tailwind Vite plugin | 4.3.5 | MIT |
| Vite / React Vite plugin | 8.3.8 / 6.3.0 | MIT |
| TypeScript | 6.0.3 | Apache-2.0 |
| React / React DOM type definitions | 19.3.0 | MIT |
| Tauri JavaScript API and CLI | 2.12.4 | Apache-2.0 OR MIT |
| Tauri / tauri-build | 2.12.5 / 2.7.4 | Apache-2.0 OR MIT |
| Tauri autostart / single-instance | 2.7.0 / 2.5.5 | Apache-2.0 OR MIT |
| serde / serde_json | 1.0.229 / 1.0.154 | MIT OR Apache-2.0 |
| reqwest / chrono | 0.13.5 / 0.4.45 | MIT OR Apache-2.0 |
| rss | 2.1.2 | MIT/Apache-2.0 (legacy dual-license spelling) |
| dom_query / rusqlite | 0.28.0 / 0.42.0 | MIT |
| zeroize | 1.9.1 | Apache-2.0 OR MIT |
| tokio / tokio-tungstenite | 1.54.1 / 0.30.0 | MIT |
| futures-util / base64 / sha2 | 0.3.34 / 0.23.1 / 0.10.9 | MIT OR Apache-2.0 |
| rodio | 0.23.0 | MIT OR Apache-2.0 |
| windows / windows-core | 0.62.2 | MIT OR Apache-2.0 |

The exact manifest metadata can be inspected in installed npm `package.json` files and the downloaded crates' `Cargo.toml` files for these locked versions. Full license files take precedence over a summary table; `OR` permits an available alternative, while `AND` requires satisfying both applicable sets of terms.

### Transitive obligations that need attention

| Components found | Declared terms | Publication relevance |
| --- | --- | --- |
| Symphonia and its MP3/PCM/RIFF/core/metadata crates, 0.5.5 | MPL-2.0 | Used through audio decoding; preserve notices and tell executable recipients where the corresponding MPL-covered source is available |
| cssparser 0.37.0/0.38.0, cssparser-macros 0.7.1, selectors 0.38.0/0.41.0, dtoa-short 0.3.5, option-ext 0.2.0 | MPL-2.0 | Present in the Rust lockfile; assess target/build/runtime use when assembling notices |
| lightningcss and platform packages, 1.33.0 | MPL-2.0 | Build tooling; using it to transform CSS is distinct from redistributing the tool itself |
| brotli 9.1.1; dpi 0.1.2; encoding_rs 0.8.44; unicode-ident 1.0.27 | Combined BSD/MIT/Apache/Unicode terms, depending on package | Preserve additional notices; do not collapse an `AND` expression into one permissive choice |
| ICU/Unicode packages | Unicode-3.0 | Retain applicable copyright and permission notices for included code/data |
| graceful-fs 4.2.12 | BlueOak-1.0.0 | Preserve the required license notice when redistributing covered software |
| target-lexicon 0.12.16 | Apache-2.0 WITH LLVM-exception | Use the actual license with its exception, not an incomplete generic notice |
| SQLite bundled via libsqlite3-sys | SQLite core is public domain; Rust bindings have separate terms | Do not infer that rusqlite's MIT declaration covers every bundled file |

Primary references: [Symphonia 0.5.5 source](https://docs.rs/crate/symphonia/0.5.5/source/), [Lightning CSS license](https://github.com/parcel-bundler/lightningcss/blob/master/LICENSE), [Mozilla's MPL distribution FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/), [SQLite copyright statement](https://www.sqlite.org/copyright.html), and [Blue Oak license](https://blueoakcouncil.org/license/1.0.0).

MPL does not automatically require all AraBrief files to become MPL-covered merely because the executable links an MPL library. Its covered-source availability and notice obligations still apply to the library. See Mozilla's FAQ for the distinction.

Before distributing installers, assemble an artifact-specific third-party notice set with required license texts, copyright notices, any relevant upstream `NOTICE` content, and durable source locations for the exact MPL-covered versions (including modifications, if any). This review is not a substitute for that notice set. The existing repository has no complete third-party notices bundle, and the contents of the published installer were not verified. No existing release was modified.

## Project assets and external content

- `src/assets/brand/` and `src-tauri/icons/`: branding and application/tray icons are tracked, but no separate authorship grant or provenance record was found. Existing design notes describe supplied brand assets; they do not establish copyright ownership. Selecting MIT does not establish rights the project does not hold; the owner must confirm the right to publish and license externally supplied assets.
- UI icons are implemented in `src/app/components/Icon.tsx`. No third-party icon package or attribution was identified; this does not establish authorship by itself.
- Fonts are referenced by system font-family names, including fallback names inside SVGs. No font binaries or remote font downloads were found among tracked assets. Do not add font files without their redistribution terms.
- The RSS fixture uses synthetic entries and example domains; the sound test is generated locally. No real user database or recording is tracked.
- News articles and publisher logos/content are not licensed by an AraBrief code license. Retrieving a page or generating a paraphrase does not establish permission to redistribute its full text. Review publisher-specific restrictions and [Google's service terms](https://policies.google.com/terms) for intended use; RSS availability is not a blanket content license.
- OpenAI API use and generated material are subject to applicable [OpenAI service terms](https://openai.com/policies/services-agreement/), independently of dependency licenses. Do not promise ownership or unrestricted redistribution of third-party source material solely because it passed through an AI model.
- WebView2 is a Microsoft runtime, not covered by AraBrief's MIT license. The current installer downloads the bootstrapper when required; [Microsoft's distribution guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution) applies.

Recheck this inventory when lockfiles, enabled features, bundled resources, or runtime distribution choices change.
