module "example" {
  source = "./modules/example"

  map_mixed = {
    "flag" = true

    "block" = {
      name = "Block"
    }
  }

  map_with_commas = {
    "alpha" = {
      name  = "Alpha"
      value = "one"
    }

    "beta" = {
      name  = "Beta"
      value = "two"
    }
  }

  map_without_commas = {
    "alpha" = {
      name  = "Alpha"
      value = "one"
    }

    "beta" = {
      name  = "Beta"
      value = "two"
    }
  }
}
