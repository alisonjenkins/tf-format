locals {
  alpha = [for x in var.items:x]
  charlie = alltrue([
    for rule in var.security_rules:
    (rule.x == null)
  ])
  delta = [for x in var.items:x if x > 0]
}
