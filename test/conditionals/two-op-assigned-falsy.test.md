---
description = "Two-operand falsy produces Strikethrough assigned to variable"
expect_output = "~~5~~"
---
# Main
1. x = false ? 5
2. **{x}**
