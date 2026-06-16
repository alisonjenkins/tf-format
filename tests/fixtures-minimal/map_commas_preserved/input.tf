module "example" {
  source = "./modules/example"

  map_with_commas = {
    "alpha" = {
      name  = "Alpha"
      value = "one"
    },
    "beta" = {
      name  = "Beta"
      value = "two"
    },
  }
}
