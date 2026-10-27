# Notices

`src-tauri/src/services/voice_service.rs` ports the dictation pipeline of lcp (`src/voice.rs`), Copyright Martin Ottersland. The audio front end, the silence phrase and credit lists, the loop, script and log probability filters, the partial agreement, the span joining and the correction acceptance rule are kept close to the original; the settings, events, model setup and cleanup through Chatnip's intelligence connections are written for Chatnip.

## Bundled data

- Emoji names and shortcodes come from [emojibase-data](https://github.com/milesj/emojibase) (MIT), compiled into the binary at build time by `src-tauri/build.rs`.
- The world map in Statistics uses [world-atlas](https://github.com/topojson/world-atlas) and [topojson-client](https://github.com/topojson/topojson-client) (ISC).
- Icons come from [Iconify](https://iconify.design) sets, each under its own open license (Lucide is ISC, the brand logos follow their set's license).

## Not bundled

- GeoLite2 location data (CC BY-SA 4.0, MaxMind) is supplied by the user.
- Whisper speech models are downloaded on first use from Hugging Face under their own licenses.
