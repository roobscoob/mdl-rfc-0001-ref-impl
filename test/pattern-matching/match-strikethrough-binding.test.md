---
description = "Strikethrough pattern binds inner document"
expect_output = "struck"
---
# Main
1. x = ~~hello~~
2. result = match x
    - ~~doc~~: "struck"
    - otherwise: "normal"
3. **{result}**
