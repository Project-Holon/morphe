# morphe

Shared code for the editors of the native Pokemon Black and White port: kenon (the modding suite) and the eidos launcher. It has two parts. Neither one reads game data.

## Interface package (`ui/`)

A Svelte 5 package. It ships `.svelte`, `.ts` and `.css` sources; the app's Vite compiles them (the `svelte` export condition).

- `morphe/theme.css`: colours, buttons, inputs, list rows. Night (Zekrom), day (Reshiram) and system themes, chosen on `<html data-theme>`.
- `morphe/theme`: `initTheme`, `setTheme`, `savedTheme`, `themeStorageKey`. An app names its own storage key before `initTheme`.
- `ThemePicker.svelte`: a plain theme list.
- `ContextMenu.svelte` and `morphe/menu.svelte`: right-click menus, toasts, the text-field menu fallback.
- `SearchSelect.svelte`: a drop-down with a search box for long lists.
- `morphe/cache.svelte`: `once`, `forget`, `debounced`.
- `morphe/dialog`: `closeOnBackdrop`.

```js
import "morphe/theme.css";
import { initTheme, themeStorageKey } from "morphe/theme";
import SearchSelect from "morphe/SearchSelect.svelte";
```

Check: `npm install && npm run check`.

## Rust workspace (`crates/`)

- `eidos-mod`: mod folders without a ROM and without a UI. The manifest (`eidos_mod.txt`), settings, hook lines, rule files and the rule checker, tables, conflicts between mods, the options grammar, `.bw` packages with safety limits, and the code-mod SDK (header, contract, module check). `tools/mod-names.mjs` regenerates `src/modnames/rules.txt` from an eidos checkout.
- `releases`: lists a GitHub repository's releases through the public API, picks an asset by name pattern, downloads it and checks its SHA-256. A token is optional (private repositories).

Check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`. Set `CARGO_TARGET_DIR` to keep build output out of the checkout.

## Using it

Apps pin a tag, never a branch.

```toml
# Cargo.toml
eidos-mod = { git = "https://github.com/Project-Holon/morphe", tag = "v0.1.0" }
releases = { git = "https://github.com/Project-Holon/morphe", tag = "v0.1.0" }
```

```json
"morphe": "github:Project-Holon/morphe#v0.1.0"
```

While the repository is private, Cargo needs `net.git-fetch-with-cli = true` so it uses the system git credentials.

A change that is wrong after tagging gets a new tag. Tags do not move.

## License

MIT.
