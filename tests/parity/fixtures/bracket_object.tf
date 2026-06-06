locals {
  rules = [{
    name = "a"
    ports = [{
      from = 80
      to   = 80
    }]
  }]
}
