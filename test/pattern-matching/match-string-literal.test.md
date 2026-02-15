---
description = "Match on string literal"
expect_output = "greeting"
---
# Main
1. x = match "hi"
    - "hi": "greeting"
    - "bye": "farewell"
    - otherwise: "unknown"
2. **{x}**
