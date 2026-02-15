---
description = "Bold inside strikethrough is not executed (no print side effect)"
expect_output = "done"
---
# Main
1. x = ~~**"should not print"**~~
2. **{"done"}**
