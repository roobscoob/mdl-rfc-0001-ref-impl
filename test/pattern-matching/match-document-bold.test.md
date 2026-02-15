---
description = "Match bold document pattern"
expect_output = "hello"
---
# Main
1. doc = [](#BoldDoc)
2. result = match doc
    - **{value}**: value
    - otherwise: "not bold"
3. **{result}**

## BoldDoc
**hello**
