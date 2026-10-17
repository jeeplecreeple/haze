# Haztify guide

This guide takes you from download to your first DJ set. You need **Spotify Premium**, because
Spotify only lets Premium accounts stream audio through third-party apps.

> ⚠️ **macOS and Windows builds are not tested yet.** They compile in CI, but nobody has played
> music with them so far. If you try one, please
> [open an issue](https://github.com/jeeplecreeple/haztify/issues/new) saying whether it worked,
> even if it did. Linux is the platform it was developed on.

- [1. Install](#1-install)
- [2. Create your Spotify app](#2-create-your-spotify-app-one-time)
- [3. Log in](#3-log-in)
- [4. Find your music](#4-find-your-music)
- [5. Set up the DJ](#5-set-up-the-dj)
- [6. Lyrics](#6-lyrics)
- [7. Make it yours](#7-make-it-yours)
- [8. Troubleshooting](#8-troubleshooting)

## 1. Install

Download the file for your system from the
[Releases page](https://github.com/jeeplecreeple/haztify/releases/latest).

### Linux

| Distro | File | How |
| --- | --- | --- |
| Debian, Ubuntu, Mint, Pop!_OS | `haztify-linux-x86_64.deb` (or `-arm64`) | `sudo apt install ./haztify-linux-x86_64.deb` |
| Arch, Manjaro, EndeavourOS, CachyOS | see [Arch Linux](#arch-linux) below | `makepkg -si` |
| Fedora, openSUSE, anything else | `haztify-linux-x86_64.tar.gz` | Extract it and run `./haztify/haztify` |

Linux needs ALSA (`alsa-lib` / `libasound2`), which almost every desktop already has. PipeWire and
PulseAudio both work through it.

### Arch Linux

There are two ways in. Both give you a normal pacman package, so `pacman -R haztify` removes it.

**Build from source** (works today, takes a few minutes the first time):

```sh
sudo pacman -S --needed base-devel git rust
git clone https://github.com/jeeplecreeple/haztify
cd haztify/packaging/arch
makepkg -si
```

`makepkg -si` compiles Haztify, runs its tests, installs any missing dependencies, and installs the
package. Haztify then shows up in your app launcher. If you use `rustup` instead of the `rust`
package, skip `rust` in the first line.

To update later:

```sh
cd haztify && git pull
cd packaging/arch && makepkg -si
```

**Prebuilt binary** (no compiling): `packaging/arch/haztify-bin/PKGBUILD` downloads the release build
instead. Copy it into an empty folder and run `makepkg -si` there. This needs the GitHub repo to be
public, because it downloads from the Releases page.

For spoken DJ intros with the system voice, also run `sudo pacman -S espeak-ng`.

### macOS (untested)

1. Download `haztify-macos-arm64.zip` for Apple Silicon (M1 and later) or `haztify-macos-x86_64.zip`
   for Intel Macs, and unzip it.
2. Drag **Haztify.app** into **Applications**.
3. The app isn't notarized by Apple, so the first launch is blocked. Right-click **Haztify.app**,
   choose **Open**, then **Open** again. If macOS still refuses, run this once in Terminal:
   `xattr -dr com.apple.quarantine /Applications/Haztify.app`

### Windows (untested)

1. Download `haztify-windows-x86_64.zip` and extract it anywhere, for example into
   `C:\Users\<you>\Apps\haztify`.
2. Run `haztify.exe`. SmartScreen may warn that the app is unrecognised, because it isn't signed.
   Choose **More info**, then **Run anyway**.

### Build it yourself

See [Building](README.md#building) in the README. Always build with `cargo build --release --locked`.

## 2. Create your Spotify app (one time)

Haztify logs in through a free developer app that belongs to you, so no one else's app or server sits
between you and Spotify.

1. Go to <https://developer.spotify.com/dashboard> and log in.
2. Click **Create app**. Fill in any name and description, for example "Haztify".
3. Under **Redirect URIs**, add exactly `http://127.0.0.1:8898/login` and click **Add**.
4. Under **Which API/SDKs are you planning to use?**, tick **Web API** and **Web Playback SDK**.
5. Save, open the app's **Settings**, and copy the **Client ID**.

The Client ID isn't a password; Haztify uses a login method (PKCE) that doesn't need a client secret.

## 3. Log in

1. Start Haztify and paste the Client ID into the welcome screen.
2. Click **Save and log in**. Your browser opens Spotify's login page; approve the permissions.
3. When the browser says "Haztify is logged in", go back to the app.

Haztify remembers the login, so later launches go straight to your library. Use **Settings → Log
out of Spotify** to forget it.

## 4. Find your music

- **Liked Songs** and your **playlists** are in the sidebar.
- **Search** (`Ctrl+F`) finds songs.
- Click a song to play it and the rest of the list after it.
- **↪** plays a song next; **+** adds it to the end of the queue.
- The **Up next** panel (`Ctrl+B`) shows the queue. Click a song there to jump to it, or **✕** to remove it.
- The player bar has shuffle, previous, play/pause, next, repeat (off, all, one), the seek bar and volume.

## 5. Set up the DJ

The DJ uses an AI model you choose, with your own API key, to pick songs and talk between them.

### Pick a provider and add a key

Open **Settings → DJ**, choose a **Provider**, then either paste a key into **API key** or leave it
empty and set the environment variable shown.

| Provider | Where to get a key | Environment variable |
| --- | --- | --- |
| OpenAI | <https://platform.openai.com/api-keys> | `OPENAI_API_KEY` |
| Anthropic (Claude) | <https://console.anthropic.com/settings/keys> | `ANTHROPIC_API_KEY` |
| Google Gemini | <https://aistudio.google.com/apikey> | `GEMINI_API_KEY` |

Any **OpenAI-compatible** service works too: choose "OpenAI / compatible", set **API base URL**, and
set **Model** to one of that service's model names.

| Service | API base URL |
| --- | --- |
| OpenRouter | `https://openrouter.ai/api/v1` |
| Groq | `https://api.groq.com/openai/v1` |
| Mistral | `https://api.mistral.ai/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Ollama (local, free, no key) | `http://localhost:11434/v1` |
| LM Studio (local, free, no key) | `http://localhost:1234/v1` |

Leave **Model** empty to use the provider's default. Click **Save settings** when you're done.

On macOS, apps opened from Finder don't see environment variables set in your shell, so paste the
key into Settings instead.

### Start a set

1. Open **DJ** in the sidebar.
2. Optionally type a mood, like "rainy Sunday jazz" or "2000s pop-punk, no ballads".
3. Click **Start DJ** (or press `Ctrl+D`).

The DJ looks at your top tracks, top artists and recent plays, picks a set, and starts playing. Its
songs are marked **✦** in the queue, and it tops the queue up by itself. Type a new mood and click
**New set** to change direction, or **Stop** to switch it off.

### What it does

**What the DJ does** has three choices:

- **Only queue songs**: it picks music and stays quiet.
- **Queue + text intros**: it writes short radio-style intros, shown under **On air**.
- **Queue + spoken intros**: it reads the intros aloud over the music.

The other DJ settings:

| Setting | Effect |
| --- | --- |
| Songs per set | How many songs it adds each time |
| Refill when queue has | It asks for more when this few songs are left |
| Talk every Nth song | 1 = before every song, 3 = every third song |
| Max words per intro | Keeps it brief |
| Discovery | 0 = your favourites, 100 = all new to you |
| Personality, DJ name, Language | Its voice and style |
| Explicit songs | Off filters explicit tracks out |

### Spoken intros

- **System voice** (default) uses your computer's built-in speech: `say` on macOS, Windows' own
  voices, and on Linux `espeak-ng` or `spd-say` (install one, e.g. `sudo apt install espeak-ng`).
  **Voice** takes a voice name your system knows, or leave it empty for the default.
- **OpenAI-compatible TTS** sounds much more natural. It uses your OpenAI key, and **Voice** can be
  `alloy`, `nova`, `onyx`, `shimmer` and others.
- **Music volume while talking** lowers the music under the voice. Set it to 0 to pause instead.

### What it costs

Each set is one request to your provider plus a few Spotify searches. With a small model that is a
fraction of a cent. Larger models cost more per set; check your provider's pricing. Local models
through Ollama or LM Studio are free.

## 6. Lyrics

Click **♪** in the player bar, **♪ Lyrics** in the sidebar, or press `Ctrl+L`. Press it again to go
back to where you were.

- The line being sung is bold, and each word lights up as it's sung.
- Click any line to jump the song there.
- Long instrumental breaks show three dots that fill up until the singing starts again.
- **⛶ Full screen** hides everything but the lyrics and the player bar. `Esc` leaves it.

Haztify asks Spotify for the lyrics first and then [LRCLIB](https://lrclib.net), a free community
database, so songs Spotify has no lyrics for often still get them. When only plain lyrics exist,
they're shown without the highlight.

**Settings → Lyrics** has the options: word-by-word highlight, the blurred cover background,
left or centred lines, the lyrics font and size, how many past lines stay on screen, and a timing
offset if the lyrics run early or late for you.

## 7. Make it yours

**Settings → Appearance and Layout** covers the common options and applies them immediately; click
**Save settings** to keep them. Everything else is in the config file, whose path is shown at the
top of Settings:

| OS | Config file |
| --- | --- |
| Linux | `~/.config/haztify/config.toml` |
| macOS | `~/Library/Application Support/haztify/config.toml` |
| Windows | `%APPDATA%\haztify\config\config.toml` |

[`config.example.toml`](config.example.toml) lists every option with comments. Some favourites:

```toml
[appearance]
theme = "apple-music-dark"  # haztify, spotify, apple-music, apple-music-dark, nord, gruvbox,
                            # catppuccin, dracula, light, or your own theme file
font_family = "Inter"       # any installed font, or one you added
compact = true

[appearance.colors]
accent = "#1db954"          # override any theme colour

[layout]
sidebar = "right"
player_bar = "top"
track_columns = ["title", "artist", "duration"]

[keys]
play_pause = "space"
```

Font family, text size and window size apply after a restart. Edit the file while Haztify is closed,
because the Settings page saves over it.

### Your own theme

**Settings → Appearance → Open folder** opens the folder next to the config file. Make a `themes`
folder there (Haztify creates it the first time you click the button) and add a file such as
`themes/ocean.toml`:

```toml
base = "apple-music-dark"   # the built-in theme to start from
background = "#0b1d2e"
surface = "#10263a"
accent = "#0a84ff"
# also: text, muted_text, danger
```

It then shows up in the **Theme** list as `ocean`. Turn on **Accent from cover art** to have the
accent colour follow whatever is playing.

### Your own fonts

Click **Add font file…** and pick a `.ttf` or `.otf` file. Haztify copies it into its `fonts`
folder and adds it to the **App font** and **Lyrics font** lists. The lyrics font changes right
away; the app font changes after a restart. Any font installed on your system works too: type its
name under **…or an installed font**.

### Your own app icon

**Settings → Appearance → App icon → Choose image…** takes a PNG or JPG (square works best). It
becomes the window and taskbar icon straight away. On Linux Haztify also writes a menu entry for
you (`~/.local/share/applications/haztify.desktop`) so the launcher and Wayland docks use it too.
On macOS this only changes the window icon; to change the Dock icon, select Haztify.app in Finder,
choose **Get Info**, and paste your image onto the small icon at the top. **Reset** goes back to
the built-in icon.

### Your own playlist covers

Open a playlist or Liked Songs and click **Change cover…** to use any image as its cover in
Haztify. This only changes it in Haztify, not on Spotify. **Reset cover** puts Spotify's back.

## 8. Troubleshooting

| Problem | Fix |
| --- | --- |
| "INVALID_CLIENT: Invalid redirect URI" in the browser | The redirect URI on your Spotify app must be exactly `http://127.0.0.1:8898/login`. |
| Login says the address is in use | Something else uses port 8898. Close it, or change `redirect_uri` in the config and on your Spotify app to another port. |
| "Premium is required" or nothing plays | Playback needs Spotify Premium on the account you logged in with. |
| Spotify returns 403 for your account | In development mode, your Spotify app only works for accounts listed under **User Management** on the dashboard. Add your account's email there. |
| "No API key for the DJ" | Add the key in Settings, or set the environment variable from the table above and restart. |
| DJ says its picks couldn't be found | The model named songs Spotify doesn't have. Try again, or try a larger model. |
| No spoken intros on Linux | Install `espeak-ng` or `speech-dispatcher`, or switch to OpenAI-compatible TTS. |
| **Choose image…** or **Add font file…** does nothing (Linux) | The file picker comes from your desktop's portal. GNOME and KDE have one; on other setups install `xdg-desktop-portal` plus a backend such as `xdg-desktop-portal-gtk`. |
| No lyrics for a song | Neither Spotify nor LRCLIB has them. Lyrics that run early or late can be fixed with **Timing offset** in Settings → Lyrics. |
| Window is blank or flickers | Set `renderer = "cpu"` under `[appearance]` (the default). If you set `"gpu"`, your graphics driver may not support it. |
| Something else | Run Haztify from a terminal with `RUST_LOG=haztify=debug` to see what it's doing, and include that output in an [issue](https://github.com/jeeplecreeple/haztify/issues/new). |
