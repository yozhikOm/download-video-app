# Changelog

## [0.2.0](https://github.com/yozhikOm/download-video-app/compare/v0.1.0...v0.2.0) (2026-11-04)

### Features

* advanced mode: download any one of a video's formats, and save the transcript as srt too ([#15](https://github.com/yozhikOm/download-video-app/issues/15))

### Bug Fixes

* show the download percent from the start when a site tells only the formats' bitrate, as vk does ([#52](https://github.com/yozhikOm/download-video-app/issues/52))
* disable the find button while its url is probed ([#54](https://github.com/yozhikOm/download-video-app/issues/54))

## [0.1.0](https://github.com/yozhikOm/download-video-app/releases/tag/v0.1.0) (2026-10-28)

First working version: paste a video URL and save its video, audio or transcript to disk.

### Features

* download and verify yt-dlp, ffmpeg and deno on first run, and keep yt-dlp up to date on its own ([#2](https://github.com/yozhikOm/download-video-app/issues/2), [#22](https://github.com/yozhikOm/download-video-app/issues/22), [#11](https://github.com/yozhikOm/download-video-app/issues/11))
* probe a url and show the video card, explaining youtube's bot check and a truncated url ([#3](https://github.com/yozhikOm/download-video-app/issues/3), [#24](https://github.com/yozhikOm/download-video-app/issues/24), [#27](https://github.com/yozhikOm/download-video-app/issues/27))
* sign in to youtube through a chosen browser's cookies ([#25](https://github.com/yozhikOm/download-video-app/issues/25))
* download a video in the default quality or a chosen video preset ([#4](https://github.com/yozhikOm/download-video-app/issues/4), [#5](https://github.com/yozhikOm/download-video-app/issues/5))
* download a video's sound alone as m4a or mp3 ([#6](https://github.com/yozhikOm/download-video-app/issues/6))
* save a video's transcript as text, alone or with media ([#12](https://github.com/yozhikOm/download-video-app/issues/12))
* run downloads one at a time from a queue ([#7](https://github.com/yozhikOm/download-video-app/issues/7))
* cancel a download and retry it, resuming from its partial ([#8](https://github.com/yozhikOm/download-video-app/issues/8))
* tell why a download failed and copy its details ([#10](https://github.com/yozhikOm/download-video-app/issues/10))
* ask before closing the app with unfinished downloads, and stop the tools with the app ([#9](https://github.com/yozhikOm/download-video-app/issues/9), [#36](https://github.com/yozhikOm/download-video-app/issues/36))
* settings: save folder, file name template and default preset ([#13](https://github.com/yozhikOm/download-video-app/issues/13))
* follow the windows theme with windows 11 colours and layout ([#42](https://github.com/yozhikOm/download-video-app/issues/42), [#46](https://github.com/yozhikOm/download-video-app/issues/46))
* ship as an nsis installer attached to the release ([#16](https://github.com/yozhikOm/download-video-app/issues/16))
