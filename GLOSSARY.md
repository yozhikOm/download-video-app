# Download Video App

A desktop app that turns a video URL from any site yt-dlp supports into files on disk: the video, its audio, or its transcript. YouTube is the primary case.

## Language

### Source

**Video**:
The remote media item a URL points to on a hosting site. A URL that points to a playlist or channel is not a Video.
_Avoid_: Clip, media, item

**Probe**:
Asking the site what a Video offers (title, duration, available qualities, transcripts) before anything is downloaded.
_Avoid_: Fetch info, analyze, scan

### What the user asks for

**Download**:
One request in the Queue to save a Video to disk: its media with a chosen Preset (or, in the Advanced mode, as one of its Formats), its Transcript, or both.
_Avoid_: Job, task

**Queue**:
The ordered list of Downloads in the current app session; Downloads in it run one at a time.
_Avoid_: List, history

**Preset**:
A user-facing choice of what to save, grouped as Video presets (by quality) or Audio presets (by format), offered only when the probed Video supports it.
_Avoid_: Profile, option, mode

**Default Preset**:
The Preset chosen at first after a Probe, one per group, set by the user: a Video preset, or the nearest lower one the Video has, and an Audio preset. The user can choose another before adding the Download.
_Avoid_: Default quality, default format, качество по умолчанию

**Format**:
One raw stream variant that the site reports for a Video (codec, resolution, bitrate). Users only see Formats in the Advanced mode, where a Download may take one exactly as it is instead of a Preset; Presets are built on top of them.
_Avoid_: Stream, itag, quality

**Partial**:
The already-received data of an unfinished Download, kept so that a later Download of the same Video with the same Preset or Format resumes instead of starting over. Partials untouched for 7 days are discarded.
_Avoid_: .part file, temp file, cache, недокачанное

**Advanced mode**:
The part of the app for experienced users, hidden until they turn it on in the settings: a Video's Formats to download one of them instead of a Preset, and saving a Transcript as `.srt` with its timing next to its text.
_Avoid_: Expert mode, pro mode, advanced view

**Transcript**:
The text of what is spoken in a Video, saved as a file alongside the media or on its own, and in the Advanced mode as `.srt` with its timing too. Each Transcript has a source; today the only source is the subtitles or auto-generated captions the site provides.
_Avoid_: Subtitles, captions, расшифровка

### Where files go

**Save folder**:
The folder, chosen by the user, that a Download's files go into as it becomes Done, so a change applies to every Download not yet Done; until they choose one, it is the system's Downloads folder. Partials are never in it.
_Avoid_: Download folder, output folder, destination

**File name template**:
The pattern, set by the user, that a Download's files are named by as it becomes Done; the app puts the Video's title, channel and upload date into it.
_Avoid_: Output template, naming pattern, mask

### Download status

A Download is in exactly one status at a time.

**Queued**:
Waiting in the Queue for the Download ahead of it to finish.

**Downloading**:
Receiving the Video's data from the site, one part after another: the Transcript, then the video and the audio, which may come as separate Formats.

**Processing**:
Turning the received data into the final file, such as merging video and audio or converting to MP3.
_Avoid_: Converting, merging, post-processing

**Done**:
The final files are on disk: the media, the Transcript, or both.
_Avoid_: Completed, finished

**Failed**:
Stopped because of an error; its Partial is kept, so retrying resumes where it stopped.
_Avoid_: Error, broken

**Cancelled**:
Stopped by the user; its Partial is removed, so retrying starts from scratch.
_Avoid_: Aborted, stopped

### Tools

**Tool**:
One of the programs the app runs from its own folder: yt-dlp, ffmpeg, ffprobe or deno (the JavaScript runtime yt-dlp needs for YouTube). Copies the user has installed elsewhere are never used.
_Avoid_: Binary, dependency, utility

**Package**:
One official release download that provides Tools: the yt-dlp program, the FFmpeg-Builds archive with ffmpeg and ffprobe, or the deno archive. A version and a published checksum belong to a Package, not to a Tool.
_Avoid_: Bundle, archive

**Setup**:
Getting every Tool in place before the app can be used: checking what is installed, then downloading and verifying the missing Packages. The setup screen shows it.
_Avoid_: First run, bootstrap, installation

**Broken Tool**:
A Tool that failed to start; the app runs Setup again to restore it. Unlike a Failed Download, it blocks the whole app until Setup ends.

### Signing in

**Sign-in browser**:
The browser, chosen by the user, in which they are signed in to YouTube; the app asks YouTube on behalf of that browser's account. Without one, and for other sites, the app asks without an account.
_Avoid_: Cookies browser, login, account
