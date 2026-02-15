---
description = "Two sibling sub-blocks under same parent"
expect_output = "alpha\nbeta"
---
# Main
1. [](#Alpha)
2. [](#Beta)

## Alpha
1. **{"alpha"}**

## Beta
1. **{"beta"}**
