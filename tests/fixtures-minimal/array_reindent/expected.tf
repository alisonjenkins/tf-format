locals {
  a = [
    1,
    2,
    3
  ]
  c = [
    "x", # one
    # own-line comment
    "z",
  ]
  d = [1,
    2,
  3]
}
resource "a" "b" {
  y = [
    1,
    2,
  ]
}
