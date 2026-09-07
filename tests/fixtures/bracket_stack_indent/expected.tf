locals {
  a = [{
    x = 1
    }, {
    y = 2
  }]

  b = merge([
    1,
    ], [
    2,
  ])

  c = [for x in var.l : {
    k = x
  }]

  d = {
    x = [
      1,
    ]
  }

  e = [
    [
      1,
    ],
  ]
}
