# Haze

A native Spotify client that stays out of your RAM, looks the way you want, and comes with an AI DJ
that runs on **your own** LLM API key.

- **Native, not a webapp.** Rust + [iced](https://iced.rs). No Electron, no webview, no browser engine.
  With the default CPU renderer the whole app idles at roughly 25 MB of RAM.
- **Plays audio itself.** [librespot](https://github.com/librespot-org/librespot) streams and decodes
  Spotify in-process (up to 320 kbps). Requires Spotify **Premium**.
- **AI DJ.** Picks the next songs from your taste, your history and whatever mood you type in, and
  (optionally) introduces them like a radio host, as text or out loud.
- **Bring your own model.** OpenAI, Anthropic Claude, Google Gemini, and anything OpenAI-compatible:
  OpenRouter, Groq, Mistral, DeepSeek, xAI, Together, or a local Ollama / LM Studio.
- **Synced lyrics.** A karaoke-style lyrics view in the spirit of Spicy Lyrics: words light up as
  they're sung, a blurred cover-art background, instrumental-break dots, click a line to jump there,
  full screen. Lyrics come from Spotify, with [LRCLIB](https://lrclib.net) as a fallback.
- **Looks the part.** Rounded panels, Inter type and Lucide icons built in, so it looks the same on
  Linux, Windows and macOS.
- **Customisable.** Themes (including Apple Music light and dark) or your own theme files, colours,
  an accent that follows the cover art, your own fonts, your own app icon, your own playlist
  covers, corner radius, compact rows, which side the sidebar and queue sit on, player bar on top or
  bottom, track columns, keyboard shortcuts.
- Linux (Arch, Debian, Ubuntu, Fedora, ...), Windows and macOS.

> ⚠️ **The macOS and Windows builds are NOT tested.** They compile in CI, but nobody has played
> music with them yet. If you try one, you're very welcome to
> [open an issue](https://github.com/jeeplecreeple/haze/issues/new) to say whether it works,
> even if it does. Linux is where Haze is developed.

## Download

Get the latest build from [Releases](https://github.com/jeeplecreeple/haze/releases/latest):

| System | File |
| --- | --- |
| Debian, Ubuntu and derivatives | `haze-linux-x86_64.deb` / `haze-linux-arm64.deb` |
| Any other Linux | `haze-linux-x86_64.tar.gz` / `haze-linux-arm64.tar.gz` |
| Arch Linux | `makepkg -si`, see the [Arch steps in the guide](GUIDE.md#arch-linux) |
| macOS, Apple Silicon (untested) | `haze-macos-arm64.zip` |
| macOS, Intel (untested) | `haze-macos-x86_64.zip` |
| Windows 10/11 (untested) | `haze-windows-x86_64.zip` |

**New here? The [guide](GUIDE.md) walks through install, Spotify setup, the DJ and customising.**

## First run

1. Create a free app at <https://developer.spotify.com/dashboard>.
   - Redirect URI: `http://127.0.0.1:8898/login` (exactly)
   - APIs: tick **Web API** and **Web Playback SDK**
2. Start Haze. A short animated setup asks for the app's **Client ID**, lets you pick a theme
   and the DJ's AI provider, and logs you in through the browser. Your login is cached, so next
   time it starts straight into your library.
3. To use the DJ, open **Settings → DJ**, pick a provider and add your API key (or set the usual
   environment variable such as `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` or `GEMINI_API_KEY` and
   leave the key field empty). Then hit **Start DJ**.

## The DJ

| Setting | What it does |
| --- | --- |
| What the DJ does | **Only queue songs**, **Queue + text intros**, or **Queue + spoken intros** |
| Songs per set / Refill when queue has | How many songs it adds at once, and when it tops the queue up |
| Talk every Nth song | How chatty it is |
| Discovery | 0 = your favourites, 100 = all new to you |
| Personality, name, language, max words | How it sounds |
| Explicit songs | Filter explicit tracks out |
| Voice engine | Your OS voice (`say` on macOS, SAPI on Windows, `espeak-ng`/`spd-say` on Linux) or any OpenAI-compatible `/audio/speech` API |
| Music volume while talking | Ducks the music under the voice; 0 pauses it instead |

Type a mood into the DJ box ("rainy Sunday jazz", "2000s pop-punk, no ballads") and press
**New set** to steer it. Songs the DJ adds are marked ✦ in the queue.

Spotify no longer gives new apps its recommendations API, so the DJ asks your LLM for real song
titles and finds each one with Spotify search; anything it can't find is skipped.

## Configuration

Everything lives in one TOML file (created on first run; the Settings page edits the common parts):

| OS | Path |
| --- | --- |
| Linux | `~/.config/haze/config.toml` |
| macOS | `~/Library/Application Support/haze/config.toml` |
| Windows | `%APPDATA%\haze\config\config.toml` |

See [`config.example.toml`](config.example.toml) for every option. The file is created readable by
your user only, since it can hold an API key. Prefer the environment-variable route for keys if you
sync your dotfiles.

`renderer = "cpu"` (default) uses the lightweight software renderer; `"gpu"` uses Vulkan, Metal or
DirectX via wgpu, which is smoother on big/high-refresh screens but costs more memory.

Set `HAZE_DEMO=1` (or `=tracks`, `=lyrics`, `=settings`, `=setup`) to open the UI with sample data and no login, handy
for trying themes and layouts.

## Building

Needs a recent stable Rust (install with `rustup`), plus on Linux the ALSA headers.

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libasound2-dev
# Arch
sudo pacman -S --needed base-devel alsa-lib
# Fedora
sudo dnf install gcc pkg-config alsa-lib-devel

cargo build --release --locked
./target/release/haze
```

Always build with `--locked`: the lockfile pins `vergen` to 9.0.x, which librespot 0.8's build script
needs.

Packaging: `packaging/arch/PKGBUILD` for Arch, `cargo deb` for a `.deb` (Debian/Ubuntu). The GitHub
Actions workflow builds and packages Linux (x86_64, arm64), macOS (Apple Silicon, Intel) and Windows
on every push, and pushing a `v*` tag publishes them as a GitHub release.

## Default shortcuts

`Ctrl+Space` play/pause · `Ctrl+→/←` next/previous · `Shift+→/←` seek 10 s · `Ctrl+↑/↓` volume ·
`Ctrl+F` search · `Ctrl+B` queue panel · `Ctrl+D` start the DJ · `Ctrl+L` lyrics. Media keys work too. `Ctrl` also
means `Cmd` on macOS. Rebind any of them under `[keys]`.

## Notes

- Haze never sends your Spotify login or API keys anywhere except Spotify and the provider you
  chose.
- This is an unofficial client and is not affiliated with Spotify.
- Bundled fonts: Inter (SIL Open Font License) and Lucide icons (ISC). See
  [`assets/fonts`](assets/fonts/README.md).
