---
description = "Otherwise arm without binding"
expect_output = "fallback"
---
# Main
1. x = match 999
    - 1: "one"
    - otherwise: "fallback"
2. **{x}**
