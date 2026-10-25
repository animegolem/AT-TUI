# README demo for at-tui. Rendered by VHS (https://github.com/charmbracelet/vhs)
# against demo/mock_server.py; run demo/record.sh rather than this file directly.

Output demo/at-tui-demo.gif
Output demo/at-tui-demo.mp4

Set Shell "bash"
Set FontFamily "FiraCode Nerd Font Mono"
Set FontSize 15
Set Width 1240
Set Height 780
Set Padding 16
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 10
Set Margin 20
Set MarginFill "#11111b"
Set Framerate 24
Set TypingSpeed 55ms

Hide
Type `clear && HOME="$DEMO_HOME" "$AT_TUI_BIN" --image-protocol halfblocks`
Enter
Sleep 3s
Show

# The timeline: selection bar, author colours, link cards, quiet counters
Sleep 1.8s
Type "j"
Sleep 900ms
# Media viewer
Space
Sleep 2.8s
Escape
Sleep 500ms

# A thread, drawn as a tree
Type "k"
Sleep 500ms
Type "l"
Sleep 2.2s
Type "j"
Sleep 600ms
Type "j"
Sleep 600ms
Type "j"
Sleep 1.2s
Type "h"
Sleep 700ms

# Author profile
Type "P"
Sleep 2.4s
Type "h"
Sleep 600ms

# Notifications, grouped, with the post they are about
Type "N"
Sleep 2.8s
Type "h"
Sleep 600ms

# Esc on the timeline: feeds, accounts and keys in one menu
Escape
Sleep 2.4s
Type "j"
Sleep 400ms
Type "j"
Sleep 600ms
Enter
Sleep 2.2s
Escape
Sleep 900ms
Type "k"
Sleep 300ms
Type "k"
Sleep 500ms
Enter
Sleep 1.5s

# Reply, then like
Type "r"
Sleep 900ms
Type "Tree guides and grouped notifications? Sold."
Sleep 900ms
Ctrl+S
Sleep 1.6s
Type "f"
Sleep 2.6s
