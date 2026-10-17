# Haze guide

This guide takes you from download to your first DJ set. You need **Spotify Premium**, because
Spotify only lets Premium accounts stream audio through third-party apps.

**The short version:** install Haze, make a free Spotify developer app, paste its Client ID into
Haze and log in. To use the DJ, add an API key from an AI provider in Settings.

### WARNING!
Haze was built and tested on CachyOS. Please [Open a issue](https://github.com/jeeplecreeple/haze/issues) stating if it worked or not. Even if it did work, this is still appreciated.

## Guide
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
[Releases page](https://github.com/jeeplecreeple/haze/releases/latest). OR you could follow the steps below:

### Debian, Ubuntu, Mint, Pop!_OS

Download `haze-linux-x86_64.deb` (or `haze-linux-arm64.deb` on ARM), then run this in the
folder you downloaded it to:

```sh
sudo apt install ./haze-linux-x86_64.deb
```

### Arch, Manjaro, EndeavourOS, CachyOS

This builds Haze from source into a normal pacman package. It takes a few minutes the first time.

```sh
sudo pacman -S --needed base-devel git rust
git clone https://github.com/jeeplecreeple/haze ~/haze
cd ~/haze/packaging/arch
makepkg -Csi
```

Haze then shows up in your app launcher. If you use `rustup`, leave `rust` out of the first line.
For spoken DJ intros, install Piper (`yay -S piper-tts-bin`) for a natural voice, or `espeak-ng`
for the robotic system voice. See [Spoken intros](#spoken-intros).

To update later:

```sh
cd ~/haze
git pull
cd packaging/arch
makepkg -Csi
```

There is also a auto updater in the program if you prefer that.

`git pull` only works inside the `haze` folder, which is why the first line goes there. The `-C`
starts each build fresh, which avoids errors like `can't find crate` from an older build.

To remove Haze, run `sudo pacman -R haze`.

### Fedora, openSUSE and other Linux

Download `haze-linux-x86_64.tar.gz`, extract it, and run `./haze/haze`. Haze needs ALSA
(`alsa-lib` or `libasound2`), which almost every desktop already has; PipeWire and PulseAudio both
work through it.

### macOS

1. Download `haze-macos-arm64.zip` for Apple Silicon (M1 and later) or `haze-macos-x86_64.zip`
   for Intel Macs, and unzip it.
2. Drag **Haze.app** into **Applications**.
3. Apple hasn't notarized the app, so the first launch is blocked. Right-click **Haze.app**,
   choose **Open**, then **Open** again. If macOS still refuses, run this once in Terminal:
   `xattr -dr com.apple.quarantine /Applications/Haze.app`

### Windows

1. Download `haze-windows-x86_64.zip` and extract it anywhere, for example
   `C:\Users\<you>\Apps\haze`.
2. Run `haze.exe`. SmartScreen may warn that the app isn't recognised, because it isn't signed.
   Choose **More info**, then **Run anyway**.

### Build it yourself

See [Building](README.md#building) in the README. Always build with `cargo build --release --locked`.

## 2. Create your Spotify app

You only do this once. Haze logs in through a free developer app that belongs to you, so no one
else's app or server sits between you and Spotify.

1. Go to <https://developer.spotify.com/dashboard> and log in.
2. Click **Create app**. Any name and description will do, for example "Haze".
3. Under **Redirect URIs**, add exactly `http://127.0.0.1:8898/login` and click **Add**.
4. Under **Which API/SDKs are you planning to use?**, tick **Web API** and **Web Playback SDK**.
5. Click **Save**, open the app's **Settings**, and copy the **Client ID**.

The Client ID isn't a password. Haze logs in with PKCE, which doesn't need a client secret.

## 3. Log in

The first time you start Haze, a short setup walks you through it: after the hello, pick a theme,
paste the Client ID from step 2, add a DJ key if you like (or **Set up later**), and click
**Log in with Spotify**. Your browser opens Spotify twice: once for your library and once so Haze
can play music (Spotify only streams to its own player login). Approve both, then go back to Haze. The setup links to the Spotify dashboard and has a **Copy** button for the
Redirect URI, so you can do step 2 from there too.

**Settings → Run setup again** brings the setup back whenever you want.

Haze remembers both logins, so later launches go straight to your library. **Settings → Log out
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
| Mistral | <https://console.mistral.ai/api-keys> | `MISTRAL_API_KEY` |
| Groq (free tier) | <https://console.groq.com/keys> | `GROQ_API_KEY` |
| OpenRouter (free models) | <https://openrouter.ai/keys> | `OPENROUTER_API_KEY` |
| DeepSeek | <https://platform.deepseek.com/api_keys> | `DEEPSEEK_API_KEY` |
| xAI (Grok) | <https://console.x.ai> | `XAI_API_KEY` |
| Cerebras (free tier) | <https://cloud.cerebras.ai> | `CEREBRAS_API_KEY` |
| Ollama (on your computer) | No key. Install from <https://ollama.com> and run `ollama pull llama3.2` | |

Each one already knows its address and a sensible model, so a key is all it needs.

**You'll likely get the best experience with Groq or Ollama.** In testing, OpenAI, Mistral and
Gemini often didn't work as well: they picked songs Spotify couldn't find, or were busy. Groq is
free and fast; Ollama runs on your own computer, so it's free and private too. Cerebras and
OpenRouter's free models also cost nothing. OpenRouter's
default model, `openrouter/free`, sends each request to whichever free model is available.

If a model name stops working (providers retire old ones), Haze asks the provider which models it
has and uses one of those. When a provider is overloaded, Haze waits and tries again twice before
giving up.

Any other **OpenAI-compatible** service works too: choose "OpenAI / compatible", set **API base
URL**, and set **Model** to one of that service's model names. LM Studio, for example, is
`http://localhost:1234/v1` and needs no key.

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
  one your system has. On Linux this sounds quite robotic.
- **Piper (natural, offline)** sounds far more like a person, runs on your own computer, and is
  free. It works on Linux and macOS (Windows too). Install it once:

  | System | Command |
  | --- | --- |
  | Arch | `yay -S piper-tts-bin` |
  | Debian, Ubuntu, Fedora, others | `sudo apt install pipx` (or your system's pipx package), then `pipx install piper-tts` |
  | macOS | `brew install pipx`, then `pipx install piper-tts` |

  Then choose **Piper** as the voice engine. The first time, Haze downloads a clear US English
  voice (about 60 MB) into its `voices` folder. For another voice or language, download a `.onnx`
  file **and** its `.onnx.json` from <https://huggingface.co/rhasspy/piper-voices> (you can listen to
  them at <https://rhasspy.github.io/piper-samples>), put both in the same folder, and click
  **Choose…** next to **Voice file**. If Haze can't find Piper, put its full path in **Piper
  program** (run `which piper` in a terminal to see it).
- **OpenAI-compatible TTS** uses your OpenAI key, and **Voice** can be `alloy`, `nova`, `onyx`,
  `shimmer` and others. It costs a little per intro.

**Music volume while talking** turns the music down under the voice. Set it to 0 to pause instead.

### What it costs

Each set is one request to your AI provider plus a few Spotify searches. With a small model that's
a fraction of a cent; larger models cost more, so check your provider's pricing. Local models
through Ollama or LM Studio are free.

## 6. Lyrics

Click the microphone in the player bar, **Lyrics** in the sidebar, or press `Ctrl+L`. Do it again to
go back to where you were.

- The line being sung is big and bright, each word lights up as it's sung, and a soft shine rides
  along the edge of the fill. When a new line starts, the lines glide up instead of jumping.
- Click any line to jump the song to it.
- Long instrumental breaks show three dots that fill up until the singing starts again.
- **Full screen** hides everything except the lyrics and the player bar. `Esc` leaves it.

Haze asks Spotify for lyrics first and then [LRCLIB](https://lrclib.net), a free community
database, so songs Spotify has no lyrics for often still get them. **Lyrics provider** in
**Settings → Lyrics** changes the order, or sticks to one of them. LRCLIB's timed lyrics are only
used when they're for a recording of the same length, so a live or extended version doesn't throw
the timing off; otherwise you get its plain lyrics. When only plain lyrics exist, they're shown
without the highlight.

The timing allows for the half second of sound your speakers are still to play, so lines light up
as you hear them. If a song still runs early or late, use **Timing offset**.

**Settings → Lyrics** also has: word-by-word highlight, the blurred cover background, left or
centred lines, the lyrics font and size, and how many past lines stay on screen.

## 7. Make it yours

**Settings → Appearance** and **Layout** cover the common options and apply them straight away;
click **Save settings** to keep them. Everything else is in the config file, whose path is shown at
the top of Settings:

| System | Config file |
| --- | --- |
| Linux | `~/.config/haze/config.toml` |
| macOS | `~/Library/Application Support/haze/config.toml` |
| Windows | `%APPDATA%\haze\config\config.toml` |

[`config.example.toml`](config.example.toml) lists every option with comments. A few favourites:

```toml
[appearance]
theme = "apple-music-dark"  # haze, spotify, apple-music, apple-music-dark, nord, gruvbox,
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

The app font, text size and window size apply after a restart. Close Haze before editing the
file, because saving in Settings writes over it.

### Your own theme

**Settings → Appearance → Open folder** opens Haze's config folder. Add a `themes` folder there
(Haze makes it the first time you click the button) with a file such as `themes/ocean.toml`:

```toml
base = "apple-music-dark"   # the built-in theme to start from
background = "#0b1d2e"
surface = "#10263a"
accent = "#0a84ff"
# also: text, muted_text, danger
```

It then shows up in the **Theme** list as `ocean`. Turn on **Accent from cover art** to have the
accent colour follow whatever is playing. **Cover glow background** (on by default) puts a blurred,
darkened copy of the current song's cover behind the whole window, so its colours show around and
through the panels.

### Your own fonts

Haze comes with [Inter](https://rsms.me/inter/) for the app and Inter Display ExtraBold for
lyrics, so it looks the same everywhere. Pick **System font** to use your desktop's font instead.

**Add font file…** takes a `.ttf` or `.otf` file. Haze copies it into its `fonts` folder and adds
it to the **App font** and **Lyrics font** lists. Fonts installed on your system work too: type the
name under **…or an installed font**.

### Your app icon

**Settings → Appearance → App icon** has eight built-in icons to pick from (the setup offers them
too), and **Use your own image…** takes PNG, JPEG, WebP, GIF, BMP, ICO, TIFF, TGA or QOI; square
works best. Haze reads the file itself, so a WebP saved with a .png name still works. You can also
drag an image onto the Haze window while Settings is open. The icon changes in the
window, the taskbar and the logo in the sidebar straight away. On Linux, Haze also writes a menu
entry (`~/.local/share/applications/haze.desktop`) so your launcher and dock use it too. On macOS this
only changes the window icon: to change the Dock icon, select Haze.app in Finder, choose
**Get Info**, and paste your image onto the small icon at the top. **Reset** brings back the
built-in icon.

### Updates

Haze checks GitHub for a new version when it starts (turn that off under **Settings → Updates**,
which also has **Check now**). When there is one, a strip across the top offers it:

- If you run Haze from the .tar.gz, the macOS app or the Windows zip, **Update now** downloads the
  new version and swaps it in; **Restart now** starts it.
- If a package manager installed it (the .deb or pacman), Haze can't replace its files, so
  **Download** opens the release page. On Arch, update with the `git pull` and `makepkg -Csi`
  steps above.

### Your own playlist covers

Open a playlist or Liked Songs and click **Change cover…** to use any image as its cover, or drag
an image onto the window while the playlist is open. This only changes it in Haze, not on Spotify.
**Reset cover** brings Spotify's back.

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
| Every song says it isn't available | Haze needs its second, playback login. Log out under **Settings**, log in again, and approve both browser pages. The message also shows what Spotify said, which helps in an issue. |
| The taskbar shows a generic icon (Linux) | Docks find the icon through Haze's menu entry. Packages install one; when you run Haze from the .tar.gz it writes its own to `~/.local/share/applications` the first time it starts. Log out and back in if your desktop still shows the old one. |
| Spotify returns 403 for your account | Your Spotify app is in development mode, which only allows accounts listed under **User Management** on the dashboard. Add your account's email there. |
| "No API key for the DJ" | Add the key in Settings, or set the environment variable from [the table above](#add-an-api-key) and restart. |
| The DJ says its picks couldn't be found | The model named songs Spotify doesn't have. Try again, or use a larger model. |
| No spoken intros | Haze says so once and the DJ talks on screen instead. Install Piper (see [Spoken intros](#spoken-intros)) and choose it again in Settings, or install `espeak-ng` (`sudo pacman -S espeak-ng` on Arch). |
| The DJ's picks are poor or it often fails | Switch the provider to Groq or Ollama, which usually work best. |
| "Haze couldn't open a file picker" (Linux) | The file picker comes from your desktop's portal. GNOME and KDE have one; elsewhere (Hyprland, Sway, i3) install `xdg-desktop-portal-gtk`, or `zenity`. Until then, drag the file onto the Haze window. |
| A playlist says Spotify won't let Haze open it | Spotify blocks developer apps from some playlists (its own mixes, and some made by other people). Haze retries with its playback login, which usually works; if it doesn't, save the songs to a playlist of your own. |
| No lyrics for a song | Neither Spotify nor LRCLIB has them. For lyrics that run early or late, use **Timing offset** in Settings → Lyrics. |
| `fatal: not a git repository` | Run `cd ~/haze` first, then try again. |
| `can't find crate` while building on Arch | Build with `makepkg -Csi`, which starts fresh. |
| The window is blank or flickers | Set `renderer = "cpu"` under `[appearance]` (the default). `"gpu"` needs a graphics driver that supports it. |
| Something else | Run Haze from a terminal with `RUST_LOG=haze=debug` and include what it prints in an [issue](https://github.com/jeeplecreeple/haze/issues/new). |
