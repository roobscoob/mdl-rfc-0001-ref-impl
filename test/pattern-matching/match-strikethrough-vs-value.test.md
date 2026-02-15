---
description = "Non-null value does not match strikethrough pattern"
expect_output = "has value"
---
# Main
1. x = 42
2. result = match x
    - ~~doc~~: "was null"
    - otherwise: "has value"
3. **{result}**
