locals {
  a = var.x ? {
    k = 1
    } : {
    k = 2
  }

  b = (
    var.x
  )

  c = [
    for k, v in var.m :
    upper(k)
    if v
  ]

  d = {
    for k, v in var.m : k => {
      v = v
    }
  }

  e = var.x ? [
    1,
  ] : []

  m = merge(
    var.a,
    {
      k = 1
    },
    var.b
  )

  n = concat(var.a,
  var.b)
}
