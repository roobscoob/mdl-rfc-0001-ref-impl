---
description = "Match on Strikethrough value with pattern"
expect_output = "was struck"
---
# Main
1. x = ~~hello~~
2. result = match x
    - ~~doc~~: "was struck"
    - otherwise: "normal"
3. **{result}**
