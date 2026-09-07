locals {
  a = 1 # c1
  b = { # c2
    x = 1 # c3
    yyyy = 2 # c4
  } # c5
  cc = 2 # c6
  d = [ # c7
    1, # c8
    22, # c9
  ] # c10
  e = 3 # c11
}
resource "a" "b" { # c12
  a = 1 # c13
  nested { # c14
    z = 1 # c15
  } # c16
} # c17

locals {
  aaa = 1 # c1

  bbbb = 2 # c2
  # own line
  ccc = 3 # c3
  dddddddd = 4
  eee = 5 # c5
  f = { # c6
  }
  g = 6 # c7
}
