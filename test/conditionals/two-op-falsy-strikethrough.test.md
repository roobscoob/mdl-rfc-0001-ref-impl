---
description = "Two-operand conditional falsy produces Strikethrough"
expect_output = "~~\"gone\"~~"
---
# Main
1. x = false ? "gone"
2. **{x}**
