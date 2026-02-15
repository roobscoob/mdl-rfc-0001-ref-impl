---
description = "Match emphasis document pattern"
expect_output = "hello"
---
# Main
1. doc = [](#EmDoc)
2. result = match doc
    - *{value}*: value
    - otherwise: "not italic"
3. **{result}**

## EmDoc
*hello*
