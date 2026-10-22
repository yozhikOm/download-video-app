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
One request in the Queue to save a Video to disk with a chosen Preset.
_Avoid_: Job, task

**Queue**:
The ordered list of Downloads in the current app session; Downloads in it run one at a time.
_Avoid_: List, history

**Preset**:
A user-facing choice of what to save, grouped as Video presets (by quality) or Audio presets (by format), offered only when the probed Video supports it.
_Avoid_: Profile, option, mode

**Format**:
One raw stream variant that the site reports for a Video (codec, resolution, bitrate). Users only see Formats in the advanced view; Presets are built on top of them.
_Avoid_: Stream, itag, quality

**Partial**:
The already-received data of an unfinished Download, kept so that a later Download of the same Video with the same Preset resumes instead of starting over. Partials untouched for 7 days are discarded.
_Avoid_: .part file, temp file, cache, недокачанное

**Transcript**:
The text of what is spoken in a Video, saved as a file alongside the media or on its own. Each Transcript has a source; today the only source is the subtitles or auto-generated captions the site provides.
_Avoid_: Subtitles, captions, расшифровка

### Download status

A Download is in exactly one status at a time.

**Queued**:
Waiting in the Queue for the Download ahead of it to finish.

**Downloading**:
Receiving the Video's data from the site.

**Processing**:
Turning the received data into the final file, such as merging video and audio or converting to MP3.
_Avoid_: Converting, merging, post-processing

**Done**:
The final file is on disk.
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
The browser, chosen by the user, in which they are signed in to YouTube; the app asks the site on behalf of that browser's account. Without one, the app asks the site without an account.
_Avoid_: Cookies browser, login, account
