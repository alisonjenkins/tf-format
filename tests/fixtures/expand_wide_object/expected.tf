locals {
  small = { foo = "bar" }

  default = {
    400 : "/400.html",
    403 : "/403.html",
    404 : "/404.html",
    405 : "/405.html",
    414 : "/414.html",
    416 : "/416.html",
    500 : "/500.html",
    501 : "/501.html",
    502 : "/502.html",
    503 : "/503.html",
    504 : "/504.html",
  }
}
