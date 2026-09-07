resource "test" "example" {
  a = 1 # c1
  cc = 2 # c2
  nested { # c3
    z = 1 # c4
  } # c5
}
