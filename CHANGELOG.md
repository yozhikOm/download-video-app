# Changelog

## [0.2.0](https://github.com/yozhikOm/download-video-app/compare/v0.1.0...v0.2.0) (2026-11-04)


### Features

* add the advanced mode with formats and srt transcripts ([24fff3f](https://github.com/yozhikOm/download-video-app/commit/24fff3f3336ff28a4cbd5b4b77d4c812824fd08a)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* add the best sound to a picture-only format and mark hls ([e9ba45a](https://github.com/yozhikOm/download-video-app/commit/e9ba45a76463c01bd525ea3312e77d6eac4bf1b2)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* **backend:** offer one of formats alike but for their ids ([b57a9ae](https://github.com/yozhikOm/download-video-app/commit/b57a9ae39f74626379907f0d04cc65ba1f34d40a)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* **ui:** always offer saving the transcript as srt ([5760b05](https://github.com/yozhikOm/download-video-app/commit/5760b059ae18168df8f2f4e5bbb5760971900b11)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* **ui:** name format codecs as people know them ([85bb867](https://github.com/yozhikOm/download-video-app/commit/85bb8673595d4666106d38dbad6029e6470fd5b4)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* **ui:** widen the formats table to the whole card ([00c65b9](https://github.com/yozhikOm/download-video-app/commit/00c65b9b118a7aa5b92506c841fa551210fea85e)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* расширенный режим — Formats и .srt ([e645aff](https://github.com/yozhikOm/download-video-app/commit/e645affedcaff4afaad59bdc31e184ba15090152))


### Bug Fixes

* **ui:** disable the find button while its url is probed ([010bd12](https://github.com/yozhikOm/download-video-app/commit/010bd12eebeda0da0501200ebcf9437c72e87356))
* **ui:** disable the find button while its url is probed ([d2502c3](https://github.com/yozhikOm/download-video-app/commit/d2502c3f6be4ae2044e47e6eeb7e115570caa9f9)), closes [#54](https://github.com/yozhikOm/download-video-app/issues/54)
* **ui:** show a format of a known height alone as 1080p ([e876ddf](https://github.com/yozhikOm/download-video-app/commit/e876ddf92feb93895439c6763075f30fed3a82e2)), closes [#15](https://github.com/yozhikOm/download-video-app/issues/15)
* **ytdlp:** estimate format sizes from bitrate for the percent ([8434043](https://github.com/yozhikOm/download-video-app/commit/8434043d584d7493a210541209f471e37c403283))
* **ytdlp:** estimate format sizes from bitrate for the percent ([9885151](https://github.com/yozhikOm/download-video-app/commit/9885151a274456daee44e33048e02f4e738dd4b5)), closes [#52](https://github.com/yozhikOm/download-video-app/issues/52)

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
