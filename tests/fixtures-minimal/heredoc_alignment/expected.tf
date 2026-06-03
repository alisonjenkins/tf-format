locals {
  a   = 1
  bb  = <<-EOT
    x
  EOT
  ccc = 2
}

locals {
  obj = {
    a   = 1
    bb  = <<-EOT
      x
    EOT
    ccc = 2
  }
}
