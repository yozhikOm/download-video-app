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
Stopped because of an error; the user can retry it.
_Avoid_: Error, broken

**Cancelled**:
Stopped by the user; partial files are removed and the user can retry it.
_Avoid_: Aborted, stopped
