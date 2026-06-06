locals {
  role_map = {
    for k, v in var.roles : k => {
      rolearn  = v.arn
      username = v.name
    } if v.enabled
  }
  merged = merge(var.defaults, {
    cluster = {
      name = var.name
      tags = {
        env = var.env
      }
    }
  })
}
