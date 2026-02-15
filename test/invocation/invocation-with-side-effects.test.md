---
description = "Invoked block prints, caller continues"
expect_output = "from sub\nfrom main"
---
# Main
1. [](#Sub)
2. **{"from main"}**

## Sub
1. **{"from sub"}**
