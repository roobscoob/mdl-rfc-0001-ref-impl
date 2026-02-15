---
description = "Invocation inside strikethrough is not called"
expect_output = "safe"
---
# Main
1. x = ~~[](#NeverRun)~~
2. **{"safe"}**

## NeverRun
1. **{"should not print"}**
