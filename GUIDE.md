# Haztify guide

This guide takes you from download to your first DJ set. You need **Spotify Premium**, because
Spotify only lets Premium accounts stream audio through third-party apps.

**The short version:** install Haztify, make a free Spotify developer app, paste its Client ID into
Haztify and log in. To use the DJ, add an API key from an AI provider in Settings.

> [!NOTE]
> The macOS and Windows builds haven't been tested on real machines yet. If you try one, please
> [open an issue](https://github.com/jeeplecreeple/haztify/issues/new) saying whether it worked.
> Linux is where Haztify is developed.

1. [Install](#1-install)
2. [Create your Spotify app](#2-create-your-spotify-app)
3. [Log in](#3-log-in)
4. [Play your music](#4-play-your-music)
5. [Set up the DJ](#5-set-up-the-dj)
6. [Lyrics](#6-lyrics)
7. [Make it yours](#7-make-it-yours)
8. [Keyboard shortcuts](#8-keyboard-shortcuts)
9. [Troubleshooting](#9-troubleshooting)

## 1. Install

Download the file for your system from the
[Releases page](https://github.com/jeeplecreeple/haztify/releases/latest).

### Debian, Ubuntu, Mint, Pop!_OS

Download `haztify-linux-x86_64.deb` (or `haztify-linux-arm64.deb` on ARM), then run this in the
folder you downloaded it to:

```sh
sudo apt install ./haztify-linux-x86_64.deb
```

### Arch, Manjaro, EndeavourOS, CachyOS

This builds Haztify from source into a normal pacman package. It takes a few minutes the first time.

```sh
sudo pacman -S --needed base-devel git rust
git clone https://github.com/jeeplecreeple/haztify ~/haztify
cd ~/haztify/packaging/arch
makepkg -Csi
```

Haztify then shows up in your app launcher. If you use `rustup`, leave `rust` out of the first line.
For spoken DJ intros with the system voice, also install `espeak-ng`.

To update later:

```sh
cd ~/haztify
git pull
cd packaging/arch
makepkg -Csi
```

`git pull` only works inside the `haztify` folder, which is why the first line goes there. The `-C`
starts each build fresh, which avoids errors like `can't find crate` from an older build.

To remove Haztify, run `sudo pacman -R haztify`.

### Fedora, openSUSE and other Linux

Download `haztify-linux-x86_64.tar.gz`, extract it, and run `./haztify/haztify`. Haztify needs ALSA
(`alsa-lib` or `libasound2`), which almost every desktop already has; PipeWire and PulseAudio both
work through it.

### macOS

1. Download `haztify-macos-arm64.zip` for Apple Silicon (M1 and later) or `haztify-macos-x86_64.zip`
   for Intel Macs, and unzip it.
2. Drag **Haztify.app** into **Applications**.
3. Apple hasn't notarized the app, so the first launch is blocked. Right-click **Haztify.app**,
   choose **Open**, then **Open** again. If macOS still refuses, run this once in Terminal:
   `xattr -dr com.apple.quarantine /Applications/Haztify.app`

### Windows

1. Download `haztify-windows-x86_64.zip` and extract it anywhere, for example
   `C:\Users\<you>\Apps\haztify`.
2. Run `haztify.exe`. SmartScreen may warn that the app isn't recognised, because it isn't signed.
   Choose **More info**, then **Run anyway**.

### Build it yourself

See [Building](README.md#building) in the README. Always build with `cargo build --release --locked`.

## 2. Create your Spotify app

You only do this once. Haztify logs in through a free developer app that belongs to you, so no one
else's app or server sits between you and Spotify.

1. Go to <https://developer.spotify.com/dashboard> and log in.
2. Click **Create app**. Any name and description will do, for example "Haztify".
3. Under **Redirect URIs**, add exactly `http://127.0.0.1:8898/login` and click **Add**.
4. Under **Which API/SDKs are you planning to use?**, tick **Web API** and **Web Playback SDK**.
5. Click **Save**, open the app's **Settings**, and copy the **Client ID**.

The Client ID isn't a password. Haztify logs in with PKCE, which doesn't need a client secret.

## 3. Log in

1. Start Haztify and paste the Client ID into the welcome screen.
2. Click **Save and log in**. Your browser opens Spotify's login page; approve the permissions.
3. When the browser says Haztify is logged in, go back to the app.

Haztify remembers the login, so later launches go straight to your library. **Settings → Log out
of Spotify** forgets it.

## 4. Play your music

- **Liked Songs** and your playlists are in the sidebar. **Search** finds any song.
- Click a song to play it, followed by the rest of the list.
- Each song has two buttons on the right: the bent arrow plays it next, and the list-with-a-plus
  adds it to the end of the queue.
- The queue button in the player bar (or `Ctrl+B`) shows what's playing and what's next. Click a
  song there to jump to it, or the **×** next to it to remove it. **Clear** empties the queue.
- The player bar has shuffle, previous, play/pause, next, repeat (off, all, one), the seek bar and
  volume. On the right are buttons for lyrics, the DJ and the queue.

## 5. Set up the DJ

The DJ uses an AI model you choose, with your own API key, to pick songs and talk between them.

### Add an API key

Open **Settings** (the gear at the bottom of the sidebar), choose a **Provider** under **DJ**, and
either paste a key into **API key** or leave it empty and set the environment variable below.

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

Leave **Model** empty to use the provider's default, then click **Save settings**.

On macOS, apps opened from Finder don't see environment variables from your shell, so paste the
key into Settings instead.

### Start a set

1. Click **DJ** in the sidebar.
2. If you like, type a mood, such as "rainy Sunday jazz" or "2000s pop-punk, no ballads".
3. Click **Start DJ**, or press `Ctrl+D`.

The DJ looks at your top tracks, top artists and recent plays, picks a set and starts playing. Its
picks have a small sparkle in the queue, and it tops the queue up by itself. Type a new mood and
click **New set** to change direction, or **Stop** to switch it off.

### Choose what it does

**What the DJ does** has three choices:

- **Only queue songs**: it picks music and stays quiet.
- **Queue + text intros**: it writes short radio-style intros, shown on the DJ page under **On air**.
- **Queue + spoken intros**: it reads the intros aloud over the music.

| Setting | What it changes |
| --- | --- |
| Songs per set | How many songs it adds each time |
| Refill when queue has | It picks more when this few songs are left |
| Talk every Nth song | 1 means before every song, 3 means every third song |
| Max words per intro | Keeps it brief |
| Discovery | 0 means your favourites, 100 means music that's new to you |
| DJ name, Personality, Language | Its voice and style |
| Explicit songs | Off leaves explicit tracks out |

### Spoken intros

Choose the **Voice engine** under **DJ voice**:

- **System voice** (the default) uses your computer's built-in speech: `say` on macOS, Windows' own
  voices, and `espeak-ng` or `spd-say` on Linux (install one, for example
  `sudo apt install espeak-ng`). Leave **Voice** empty for the default voice, or type the name of
  one your system has.
- **OpenAI-compatible TTS** sounds much more natural. It uses your OpenAI key, and **Voice** can be
  `alloy`, `nova`, `onyx`, `shimmer` and others.

**Music volume while talking** turns the music down under the voice. Set it to 0 to pause instead.

### What it costs

Each set is one request to your AI provider plus a few Spotify searches. With a small model that's
a fraction of a cent; larger models cost more, so check your provider's pricing. Local models
through Ollama or LM Studio are free.

## 6. Lyrics

Click the microphone in the player bar, **Lyrics** in the sidebar, or press `Ctrl+L`. Do it again to
go back to where you were.

- The line being sung is big and bright, and each word lights up as it's sung.
- Click any line to jump the song to it.
- Long instrumental breaks show three dots that fill up until the singing starts again.
- **Full screen** hides everything except the lyrics and the player bar. `Esc` leaves it.

Haztify asks Spotify for lyrics first and then [LRCLIB](https://lrclib.net), a free community
database, so songs Spotify has no lyrics for often still get them. When only plain lyrics exist,
they're shown without the highlight.

**Settings → Lyrics** has the options: word-by-word highlight, the blurred cover background, left
or centred lines, the lyrics font and size, how many past lines stay on screen, and a timing offset
for lyrics that run early or late.

## 7. Make it yours

**Settings → Appearance** and **Layout** cover the common options and apply them straight away;
click **Save settings** to keep them. Everything else is in the config file, whose path is shown at
the top of Settings:

| System | Config file |
| --- | --- |
| Linux | `~/.config/haztify/config.toml` |
| macOS | `~/Library/Application Support/haztify/config.toml` |
| Windows | `%APPDATA%\haztify\config\config.toml` |

[`config.example.toml`](config.example.toml) lists every option with comments. A few favourites:

```toml
[appearance]
theme = "apple-music-dark"  # haztify, spotify, apple-music, apple-music-dark, nord, gruvbox,
                            # catppuccin, dracula, light, or your own theme file
font_family = "Noto Sans"   # empty = the built-in Inter, "system" = your system font
compact = true

[appearance.colors]
accent = "#1db954"          # override any theme colour

[layout]
sidebar = "right"
player_bar = "top"
track_columns = ["title", "artist", "duration"]
```

The app font, text size and window size apply after a restart. Close Haztify before editing the
file, because saving in Settings writes over it.

### Your own theme

**Settings → Appearance → Open folder** opens Haztify's config folder. Add a `themes` folder there
(Haztify makes it the first time you click the button) with a file such as `themes/ocean.toml`:

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

Haztify comes with [Inter](https://rsms.me/inter/) for the app and Inter Display ExtraBold for
lyrics, so it looks the same everywhere. Pick **System font** to use your desktop's font instead.

**Add font file…** takes a `.ttf` or `.otf` file. Haztify copies it into its `fonts` folder and adds
it to the **App font** and **Lyrics font** lists. Fonts installed on your system work too: type the
name under **…or an installed font**.

### Your own app icon

**Settings → Appearance → App icon → Choose image…** takes a PNG or JPG; square works best. It
becomes the window and taskbar icon straight away. On Linux, Haztify also writes a menu entry
(`~/.local/share/applications/haztify.desktop`) so your launcher and dock use it too. On macOS this
only changes the window icon: to change the Dock icon, select Haztify.app in Finder, choose
**Get Info**, and paste your image onto the small icon at the top. **Reset** brings back the
built-in icon.

### Your own playlist covers

Open a playlist or Liked Songs and click **Change cover…** to use any image as its cover. This only
changes it in Haztify, not on Spotify. **Reset cover** brings Spotify's back.

## 8. Keyboard shortcuts

| Keys | Does |
| --- | --- |
| `Ctrl+Space` | Play or pause |
| `Ctrl+→` / `Ctrl+←` | Next or previous song |
| `Shift+→` / `Shift+←` | Skip forward or back within the song |
| `Ctrl+↑` / `Ctrl+↓` | Volume up or down |
| `Ctrl+F` | Search |
| `Ctrl+B` | Show or hide the queue |
| `Ctrl+D` | Start the DJ |
| `Ctrl+L` | Lyrics |
| `Esc` | Leave full screen |

You can change any of them under `[keys]` in the config file.

## 9. Troubleshooting

| Problem | Fix |
| --- | --- |
| "INVALID_CLIENT: Invalid redirect URI" in the browser | The redirect URI on your Spotify app must be exactly `http://127.0.0.1:8898/login`. |
| Login says the address is in use | Something else is using port 8898. Close it, or change `redirect_uri` in the config file and on your Spotify app to another port. |
| "Premium is required" or nothing plays | Playback needs Spotify Premium on the account you logged in with. |
| Spotify returns 403 for your account | Your Spotify app is in development mode, which only allows accounts listed under **User Management** on the dashboard. Add your account's email there. |
| "No API key for the DJ" | Add the key in Settings, or set the environment variable from [the table above](#add-an-api-key) and restart. |
| The DJ says its picks couldn't be found | The model named songs Spotify doesn't have. Try again, or use a larger model. |
| No spoken intros on Linux | Install `espeak-ng` or `speech-dispatcher`, or switch to OpenAI-compatible TTS. |
| **Choose image…** or **Add font file…** does nothing (Linux) | The file picker comes from your desktop's portal. GNOME and KDE have one; elsewhere install `xdg-desktop-portal` and a backend such as `xdg-desktop-portal-gtk`. |
| No lyrics for a song | Neither Spotify nor LRCLIB has them. For lyrics that run early or late, use **Timing offset** in Settings → Lyrics. |
| `fatal: not a git repository` | Run `cd ~/haztify` first, then try again. |
| `can't find crate` while building on Arch | Build with `makepkg -Csi`, which starts fresh. |
| The window is blank or flickers | Set `renderer = "cpu"` under `[appearance]` (the default). `"gpu"` needs a graphics driver that supports it. |
| Something else | Run Haztify from a terminal with `RUST_LOG=haztify=debug` and include what it prints in an [issue](https://github.com/jeeplecreeple/haztify/issues/new). |
