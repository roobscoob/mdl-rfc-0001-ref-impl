---
description = "Match strikethrough pattern catches null"
expect_output = "was null"
---
# Main
1. x = false ? 42
2. result = match x
    - ~~doc~~: "was null"
    - otherwise: "not null"
3. **{result}**
