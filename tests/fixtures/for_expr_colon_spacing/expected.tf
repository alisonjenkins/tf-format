locals {
  alpha = [for x in var.items : x]
  bravo = {for k, v in var.items : k => v}
  delta = [for x in var.items : x if x > 0]

  charlie = alltrue([
    for rule in var.security_rules :
    (rule.x == null)
  ])
}
