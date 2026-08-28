# repolish 的终端演示，由 VHS 渲染成 demo/demo.gif。
#
#   vhs demo/demo.tape
#
# 本机装不了 vhs 也没关系：`.github/workflows/demo.yml` 会在 Ubuntu runner
# 上渲染并提交结果。手动触发，不跟着每次 push 跑——GIF 是二进制大块，
# 每次 push 重渲一遍只会把历史撑肥。
#
# 演示的是一次真实的前后对比：一个故意写得很糙的仓库（21/100），
# 跑一次 polish，再看分数。没有一处是摆拍的。

Output demo/demo.gif

Set Shell bash
Set FontSize 15
Set Width 1180
Set Height 860
Set Padding 26
Set TypingSpeed 45ms

# 和 repolish 自己的配色同源，见 crates/repolish-render/src/theme.rs
Set Theme { "background": "#1a1a24", "foreground": "#edebfa", "cursor": "#ff5fd1", "selection": "#35334a", "black": "#1a1a24", "red": "#ff4f6e", "green": "#a9f05f", "yellow": "#ffc53d", "blue": "#7d56f4", "magenta": "#ff5fd1", "cyan": "#43e5d0", "white": "#edebfa", "brightBlack": "#7b7793", "brightRed": "#ff4f6e", "brightGreen": "#a9f05f", "brightYellow": "#ffc53d", "brightBlue": "#7d56f4", "brightMagenta": "#ff5fd1", "brightCyan": "#43e5d0", "brightWhite": "#ffffff" }

# 准备一个独立的样例仓库。这一段不入镜：观众要看的是工具，
# 不是我怎么摆的台子。
Hide
Type "cd $(./demo/setup.sh)" Enter
Sleep 2s
Type "clear" Enter
Sleep 500ms
Show

Type "repolish check ." Sleep 700ms Enter
Sleep 7s

Type "clear" Enter
Sleep 300ms

Type "repolish polish . --apply" Sleep 700ms Enter
Sleep 6s

Type "clear" Enter
Sleep 300ms

Type "repolish check ." Sleep 700ms Enter
Sleep 6s
