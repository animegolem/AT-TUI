# README demo

`at-tui-demo.gif` is rendered by [VHS](https://github.com/charmbracelet/vhs) from `demo.tape`, so it can be re-recorded after any UI change:

```sh
brew install vhs      # also pulls in ttyd; ffmpeg is required too
demo/record.sh
```

`record.sh` builds a release binary, starts `mock_server.py` (a stand-in for the Bluesky XRPC API, Python standard library only), logs two demo accounts into a throwaway `HOME`, and runs the tape. Your real at-tui accounts and config are never read or written.

All accounts live under the reserved `.example` domain and every post is fictional. The images are drawn by the server at startup. Writes (posting, liking) are acknowledged and then forgotten.

The tape also writes `at-tui-demo.mp4` (git-ignored). GitHub only plays video inline when it's uploaded as an attachment, so drag the MP4 into a release or PR description if you want video rather than a GIF.
