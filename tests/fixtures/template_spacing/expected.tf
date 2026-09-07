locals {
  a = "%{if var.x}yes%{else}no%{endif}"
  b = "y${var.x}z"
  c = "y%{if var.x}A%{else}B%{endif}z"
  d = "y%{~if var.x~}A%{~else~}B%{~endif~}z"
  e = "y%{for k, v in var.m~}A%{endfor}z"
  f = "${var.x}-y"
  g = "x-${var.y}"

  h = <<EOT
%{for x in var.l~}
${x}
%{endfor~}
EOT
}
