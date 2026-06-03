module "z_net" {
  source = "./net"
}

module "a_app" {
  source = "./app"
}

variable "a_var" {}

variable "z_var" {}
