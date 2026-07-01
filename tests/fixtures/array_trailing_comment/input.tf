module "example" {
  enable_services = [
    "first.example.com", // comment explaining why first is needed
    "second.example.com",
    "third.example.com", # trailing on the last element
  ]
  mixed = [
    "x",
    // genuine leading comment for y
    "y",
    "z", // trailing on z
    // leading for w
    "w",
  ]
}
