module "trailing-lines-example" {
  source = "./modules/example"

  common = local.common
  name   = "my-name"

  access_by_team = {
    "a" = "admin"


    "b" = "user"


  }



  teams = [
    "a",


    "b",


  ]


}
