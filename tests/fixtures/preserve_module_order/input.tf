module "z_net" {
  source = "./net"
}

module "a_app" {
  source = "./app"
}

variable "z_var" {}
variable "a_var" {}
