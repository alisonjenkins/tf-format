locals {
  obj = { # inline stays put
    # own line moves down
    a = 1
    b = 2
  }
  nested = {
    inner = { # nested inline
      x = 1
    }
  }
}
