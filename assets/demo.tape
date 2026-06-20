Output "demo.gif"

Require echo

Set Shell "zsh"
Set FontSize 18
Set Width 1920
Set Height 1080
Set TypingSpeed 50ms
Set FontFamily "Hack Nerd Font"
Set Padding 16
Set WindowBar Colorful
Set Framerate 20

Hide
Type "clear"
Enter
Show

# Open two Markdown files as tabs
Type `mq-tui demo.md demo2.md`
Enter
Sleep 1.5s

# Switch tabs
Tab
Sleep 1s
Tab
Sleep 1s

# Sidebar navigation
Type `s`
Sleep 1s

Type `j`
Sleep 0.5s

Type `j`
Sleep 0.5s

Type `j`
Sleep 0.5s

Type `j`
Sleep 0.5s

Type `j`
Sleep 1s

Type `k`
Sleep 0.5s

Type `k`
Sleep 0.5s

Type `k`
Sleep 0.5s

Type `k`
Sleep 0.5s

Type `k`
Sleep 1s

Type `s`
Sleep 1s

# Query mode
Type `:`
Sleep 0.5s

Type `.h`
Sleep 1.5s

Backspace 2
Sleep 1s

Type `.code`
Sleep 1.5s

Backspace 5
Sleep 1s

Type `select(.code.lang == "python")`
Sleep 1.5s

Escape
Sleep 1s

Type `p`
Sleep 1.5s

Type `j`
Sleep 0.4s

Type `j`
Sleep 0.4s

Type `j`
Sleep 0.4s

Type `j`
Sleep 0.4s

Type `j`
Sleep 1.5s

Escape
Sleep 1s

Type `q`
Sleep 0.5s
